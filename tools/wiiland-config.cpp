/*
 * WiiLand - Qt 6 configuration and diagnostics frontend
 * Optional desktop UI wrapper around the headless wiilandd daemon.
 */

#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QHash>
#include <QtCore/QIODevice>
#include <QtCore/QProcess>
#include <QtCore/QStandardPaths>
#include <QtCore/QTextStream>
#include <QtCore/QStringList>
#include <QtGui/QFont>
#include <QtWidgets/QApplication>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QFileDialog>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QGridLayout>
#include <QtWidgets/QGroupBox>
#include <QtWidgets/QHBoxLayout>
#include <QtWidgets/QHeaderView>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QMainWindow>
#include <QtWidgets/QMessageBox>
#include <QtWidgets/QPlainTextEdit>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QSpinBox>
#include <QtWidgets/QStatusBar>
#include <QtWidgets/QTableWidget>
#include <QtWidgets/QTableWidgetItem>
#include <QtWidgets/QTabWidget>
#include <QtWidgets/QVBoxLayout>
#include <QtWidgets/QWidget>

namespace {

QString defaultConfigPath()
{
    const QString configHome = QStandardPaths::writableLocation(QStandardPaths::ConfigLocation);
    if (!configHome.isEmpty())
        return configHome + QStringLiteral("/wiiland/wiilandd.conf");
    return QDir::homePath() + QStringLiteral("/.config/wiiland/wiilandd.conf");
}

QString quoteCommand(const QString &program, const QStringList &arguments)
{
    QStringList parts;
    parts << program;
    for (const QString &arg : arguments) {
        QString escaped = arg;
        escaped.replace(QStringLiteral("'"), QStringLiteral("'\\''"));
        parts << QStringLiteral("'") + escaped + QStringLiteral("'");
    }
    return parts.join(QLatin1Char(' '));
}

QStringList buttonActions()
{
    return {
        QStringLiteral("left-click"),
        QStringLiteral("right-click"),
        QStringLiteral("enter"),
        QStringLiteral("escape"),
        QStringLiteral("overview"),
        QStringLiteral("page-up"),
        QStringLiteral("page-down"),
        QStringLiteral("disabled"),
    };
}

void setComboText(QComboBox *combo, const QString &value)
{
    const int index = combo->findText(value);
    if (index >= 0)
        combo->setCurrentIndex(index);
}

} // namespace

class MainWindow final : public QMainWindow {
public:
    MainWindow()
    {
        setWindowTitle(QStringLiteral("WiiLand Control Center"));
        resize(1180, 780);

        auto *root = new QWidget(this);
        auto *rootLayout = new QVBoxLayout(root);
        rootLayout->setContentsMargins(18, 18, 18, 18);
        rootLayout->setSpacing(14);

        auto *title = new QLabel(QStringLiteral("WiiLand Wayland Control Center"), root);
        QFont titleFont = title->font();
        titleFont.setPointSize(titleFont.pointSize() + 8);
        titleFont.setBold(true);
        title->setFont(titleFont);
        rootLayout->addWidget(title);

        auto *subtitle = new QLabel(
            QStringLiteral("Configure the optional desktop/gamepad profiles, inspect runtime readiness, "
                           "and collect validation traces without putting GUI code in the input daemon."),
            root);
        subtitle->setWordWrap(true);
        rootLayout->addWidget(subtitle);

        auto *tabs = new QTabWidget(root);
        tabs->addTab(buildOverviewTab(tabs), QStringLiteral("Overview"));
        tabs->addTab(buildConfigTab(tabs), QStringLiteral("Configuration"));
        tabs->addTab(buildValidationTab(tabs), QStringLiteral("Validation"));
        rootLayout->addWidget(tabs, 1);

        setCentralWidget(root);
        statusBar()->showMessage(QStringLiteral("Ready"));
        loadConfigFromPath(defaultConfigPath(), false);
    }

private:
    QWidget *buildOverviewTab(QWidget *parent)
    {
        auto *tab = new QWidget(parent);
        auto *layout = new QVBoxLayout(tab);

        auto *paths = new QGroupBox(QStringLiteral("Command and configuration"), tab);
        auto *form = new QFormLayout(paths);
        wiilanddPath = new QLineEdit(QStringLiteral("wiilandd"), paths);
        configPath = new QLineEdit(defaultConfigPath(), paths);
        auto *browse = new QPushButton(QStringLiteral("Browse..."), paths);
        auto *configRow = new QWidget(paths);
        auto *configRowLayout = new QHBoxLayout(configRow);
        configRowLayout->setContentsMargins(0, 0, 0, 0);
        configRowLayout->addWidget(configPath, 1);
        configRowLayout->addWidget(browse);
        form->addRow(QStringLiteral("wiilandd executable"), wiilanddPath);
        form->addRow(QStringLiteral("Config file"), configRow);
        layout->addWidget(paths);

        connect(browse, &QPushButton::clicked, this, [this]() {
            const QString chosen = QFileDialog::getSaveFileName(
                this,
                QStringLiteral("Choose wiilandd configuration"),
                configPath->text(),
                QStringLiteral("Configuration files (*.conf);;All files (*)"));
            if (!chosen.isEmpty())
                configPath->setText(chosen);
        });

        auto *quick = new QGroupBox(QStringLiteral("Readiness checks"), tab);
        auto *quickLayout = new QGridLayout(quick);
        const auto addButton = [this, quickLayout](const QString &text, const QStringList &args, int row, int column) {
            auto *button = new QPushButton(text);
            quickLayout->addWidget(button, row, column);
            connect(button, &QPushButton::clicked, this, [this, args]() { runCommand(args); });
        };
        addButton(QStringLiteral("Doctor"), {QStringLiteral("--doctor")}, 0, 0);
        addButton(QStringLiteral("Check config"), {QStringLiteral("--check-config")}, 0, 1);
        addButton(QStringLiteral("Dump config"), {QStringLiteral("--dump-config")}, 0, 2);
        addButton(QStringLiteral("List devices"), {QStringLiteral("--list"), QStringLiteral("--verbose")}, 1, 0);
        addButton(QStringLiteral("Axis/button map"), {QStringLiteral("--axis-map")}, 1, 1);
        addButton(QStringLiteral("Validation checklist"), {QStringLiteral("--validation-checklist")}, 1, 2);
        layout->addWidget(quick);

        output = new QPlainTextEdit(tab);
        output->setReadOnly(true);
        output->setLineWrapMode(QPlainTextEdit::NoWrap);
        layout->addWidget(output, 1);

        return tab;
    }

    QWidget *buildConfigTab(QWidget *parent)
    {
        auto *tab = new QWidget(parent);
        auto *layout = new QHBoxLayout(tab);

        auto *profileBox = new QGroupBox(QStringLiteral("Profiles and pointer feel"), tab);
        auto *profileForm = new QFormLayout(profileBox);
        profile = new QComboBox(profileBox);
        profile->addItems({QStringLiteral("gamepad"), QStringLiteral("desktop"), QStringLiteral("both")});
        pointerSpeed = spinBox(1, 127, 16, profileBox);
        irSpeed = spinBox(1, 127, 8, profileBox);
        irDeadzone = spinBox(0, 127, 0, profileBox);
        irSmoothing = spinBox(0, 95, 0, profileBox);
        profileForm->addRow(QStringLiteral("Default profile"), profile);
        profileForm->addRow(QStringLiteral("D-pad pointer speed"), pointerSpeed);
        profileForm->addRow(QStringLiteral("IR pointer gain"), irSpeed);
        profileForm->addRow(QStringLiteral("IR jitter deadzone"), irDeadzone);
        profileForm->addRow(QStringLiteral("IR smoothing %"), irSmoothing);

        auto *bindingsBox = new QGroupBox(QStringLiteral("Desktop button bindings"), tab);
        auto *bindingsForm = new QFormLayout(bindingsBox);
        for (const QString &name : {QStringLiteral("a"), QStringLiteral("b"), QStringLiteral("plus"),
                                   QStringLiteral("minus"), QStringLiteral("home"), QStringLiteral("one"),
                                   QStringLiteral("two")}) {
            auto *combo = new QComboBox(bindingsBox);
            combo->addItems(buttonActions());
            desktopActions.insert(name, combo);
            bindingsForm->addRow(QStringLiteral("desktop.") + name, combo);
        }
        setComboText(desktopActions.value(QStringLiteral("a")), QStringLiteral("left-click"));
        setComboText(desktopActions.value(QStringLiteral("b")), QStringLiteral("right-click"));
        setComboText(desktopActions.value(QStringLiteral("plus")), QStringLiteral("enter"));
        setComboText(desktopActions.value(QStringLiteral("minus")), QStringLiteral("escape"));
        setComboText(desktopActions.value(QStringLiteral("home")), QStringLiteral("overview"));
        setComboText(desktopActions.value(QStringLiteral("one")), QStringLiteral("page-down"));
        setComboText(desktopActions.value(QStringLiteral("two")), QStringLiteral("page-up"));

        auto *deviceBox = new QGroupBox(QStringLiteral("Per-device profile rules"), tab);
        auto *deviceLayout = new QVBoxLayout(deviceBox);
        rules = new QTableWidget(0, 3, deviceBox);
        rules->setHorizontalHeaderLabels({QStringLiteral("Kind"), QStringLiteral("Match substring"), QStringLiteral("Profile")});
        rules->horizontalHeader()->setStretchLastSection(true);
        rules->verticalHeader()->hide();
        deviceLayout->addWidget(rules);
        auto *ruleButtons = new QHBoxLayout;
        auto *addRule = new QPushButton(QStringLiteral("Add rule"), deviceBox);
        auto *removeRule = new QPushButton(QStringLiteral("Remove selected"), deviceBox);
        ruleButtons->addWidget(addRule);
        ruleButtons->addWidget(removeRule);
        ruleButtons->addStretch(1);
        deviceLayout->addLayout(ruleButtons);
        connect(addRule, &QPushButton::clicked, this, [this]() { appendRule(QStringLiteral("device-type"), QString(), QStringLiteral("gamepad")); });
        connect(removeRule, &QPushButton::clicked, this, [this]() { rules->removeRow(rules->currentRow()); });

        auto *left = new QVBoxLayout;
        left->addWidget(profileBox);
        left->addWidget(bindingsBox);
        left->addStretch(1);
        layout->addLayout(left, 1);
        layout->addWidget(deviceBox, 2);

        auto *actions = new QVBoxLayout;
        auto *load = new QPushButton(QStringLiteral("Load config"), tab);
        auto *save = new QPushButton(QStringLiteral("Save config"), tab);
        auto *validate = new QPushButton(QStringLiteral("Save + check"), tab);
        actions->addWidget(load);
        actions->addWidget(save);
        actions->addWidget(validate);
        actions->addStretch(1);
        layout->addLayout(actions);
        connect(load, &QPushButton::clicked, this, [this]() { loadConfigFromPath(configPath->text(), true); });
        connect(save, &QPushButton::clicked, this, [this]() { saveConfig(); });
        connect(validate, &QPushButton::clicked, this, [this]() {
            if (saveConfig())
                runCommand({QStringLiteral("--config"), configPath->text(), QStringLiteral("--check-config")});
        });

        return tab;
    }

    QWidget *buildValidationTab(QWidget *parent)
    {
        auto *tab = new QWidget(parent);
        auto *layout = new QVBoxLayout(tab);

        auto *matrix = new QGroupBox(QStringLiteral("Hardware validation capture"), tab);
        auto *matrixLayout = new QFormLayout(matrix);
        deviceSelector = new QLineEdit(matrix);
        traceFilter = new QComboBox(matrix);
        traceFilter->addItems({QStringLiteral("all"), QStringLiteral("keys"), QStringLiteral("axes"), QStringLiteral("ir"), QStringLiteral("motion-plus")});
        matrixLayout->addRow(QStringLiteral("Device number or /sys path"), deviceSelector);
        matrixLayout->addRow(QStringLiteral("Trace filter"), traceFilter);
        layout->addWidget(matrix);

        auto *buttons = new QHBoxLayout;
        auto *startTraceButton = new QPushButton(QStringLiteral("Start dry-run trace"), tab);
        auto *stopTraceButton = new QPushButton(QStringLiteral("Stop trace"), tab);
        auto *clear = new QPushButton(QStringLiteral("Clear output"), tab);
        buttons->addWidget(startTraceButton);
        buttons->addWidget(stopTraceButton);
        buttons->addWidget(clear);
        buttons->addStretch(1);
        layout->addLayout(buttons);
        connect(startTraceButton, &QPushButton::clicked, this, [this]() { startTrace(); });
        connect(stopTraceButton, &QPushButton::clicked, this, [this]() { stopTrace(); });
        connect(clear, &QPushButton::clicked, this, [this]() { output->clear(); });

        auto *checklist = new QLabel(
            QStringLiteral("Recommended matrix: original Wii Remote, MotionPlus external and built-in, "
                           "Nunchuk, Classic Controller, Wii U Pro Controller, Guitar, Drums, Balance Board, "
                           "then SDL, Wine/Proton, and native Wayland desktop profile behavior."),
            tab);
        checklist->setWordWrap(true);
        layout->addWidget(checklist);
        layout->addStretch(1);
        return tab;
    }

    QSpinBox *spinBox(int minimum, int maximum, int value, QWidget *parent)
    {
        auto *box = new QSpinBox(parent);
        box->setRange(minimum, maximum);
        box->setValue(value);
        return box;
    }

    void appendOutput(const QString &text)
    {
        if (!output)
            return;
        output->appendPlainText(text.trimmed());
    }

    void runCommand(const QStringList &arguments)
    {
        QProcess process(this);
        process.setProcessChannelMode(QProcess::MergedChannels);
        const QString program = wiilanddPath->text().trimmed().isEmpty()
            ? QStringLiteral("wiilandd")
            : wiilanddPath->text().trimmed();
        appendOutput(QStringLiteral("$ ") + quoteCommand(program, arguments));
        process.start(program, arguments);
        if (!process.waitForStarted(5000)) {
            appendOutput(QStringLiteral("failed to start: ") + process.errorString());
            return;
        }
        process.waitForFinished(15000);
        appendOutput(QString::fromLocal8Bit(process.readAllStandardOutput()));
        if (process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0)
            appendOutput(QStringLiteral("exit status: %1").arg(process.exitCode()));
        statusBar()->showMessage(QStringLiteral("Command finished"), 4000);
    }

    void startTrace()
    {
        stopTrace();
        traceProcess = new QProcess(this);
        traceProcess->setProcessChannelMode(QProcess::MergedChannels);
        connect(traceProcess, &QProcess::readyReadStandardOutput, this, [this]() {
            appendOutput(QString::fromLocal8Bit(traceProcess->readAllStandardOutput()));
        });
        connect(traceProcess, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this, [this](int code, QProcess::ExitStatus status) {
            appendOutput(QStringLiteral("trace stopped: exit=%1 status=%2").arg(code).arg(status));
            traceProcess->deleteLater();
            traceProcess = nullptr;
        });

        QStringList args{QStringLiteral("--dry-run"), QStringLiteral("--trace-events=") + traceFilter->currentText(),
                         QStringLiteral("--verbose"), QStringLiteral("--profile"), QStringLiteral("both")};
        const QString device = deviceSelector->text().trimmed();
        if (!device.isEmpty())
            args << QStringLiteral("--device") << device;
        const QString program = wiilanddPath->text().trimmed().isEmpty()
            ? QStringLiteral("wiilandd")
            : wiilanddPath->text().trimmed();
        appendOutput(QStringLiteral("$ ") + quoteCommand(program, args));
        traceProcess->start(program, args);
        statusBar()->showMessage(QStringLiteral("Trace running"));
    }

    void stopTrace()
    {
        if (!traceProcess)
            return;
        traceProcess->terminate();
        if (!traceProcess->waitForFinished(1500))
            traceProcess->kill();
    }

    void appendRule(const QString &kind, const QString &match, const QString &ruleProfile)
    {
        const int row = rules->rowCount();
        rules->insertRow(row);
        auto *kindCombo = new QComboBox(rules);
        kindCombo->addItems({QStringLiteral("device"), QStringLiteral("device-type")});
        setComboText(kindCombo, kind);
        auto *matchItem = new QTableWidgetItem(match);
        auto *profileCombo = new QComboBox(rules);
        profileCombo->addItems({QStringLiteral("gamepad"), QStringLiteral("desktop"), QStringLiteral("both")});
        setComboText(profileCombo, ruleProfile);
        rules->setCellWidget(row, 0, kindCombo);
        rules->setItem(row, 1, matchItem);
        rules->setCellWidget(row, 2, profileCombo);
    }

    void loadConfigFromPath(const QString &path, bool reportErrors)
    {
        QFile file(path);
        if (!file.exists()) {
            if (reportErrors)
                QMessageBox::information(this, QStringLiteral("Config not found"), path);
            return;
        }
        if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
            if (reportErrors)
                QMessageBox::warning(this, QStringLiteral("Cannot read config"), file.errorString());
            return;
        }

        rules->setRowCount(0);
        QTextStream in(&file);
        while (!in.atEnd()) {
            QString line = in.readLine().trimmed();
            if (line.isEmpty() || line.startsWith(QLatin1Char('#')))
                continue;
            const int equal = line.indexOf(QLatin1Char('='));
            if (equal <= 0)
                continue;
            const QString key = line.left(equal).trimmed();
            const QString value = line.mid(equal + 1).trimmed();
            applyConfigValue(key, value);
        }
        statusBar()->showMessage(QStringLiteral("Loaded %1").arg(path), 4000);
    }

    void applyConfigValue(const QString &key, const QString &value)
    {
        if (key == QStringLiteral("profile"))
            setComboText(profile, value);
        else if (key == QStringLiteral("pointer-speed"))
            pointerSpeed->setValue(value.toInt());
        else if (key == QStringLiteral("ir-speed"))
            irSpeed->setValue(value.toInt());
        else if (key == QStringLiteral("ir-deadzone"))
            irDeadzone->setValue(value.toInt());
        else if (key == QStringLiteral("ir-smoothing"))
            irSmoothing->setValue(value.toInt());
        else if (key.startsWith(QStringLiteral("desktop.")))
            setComboText(desktopActions.value(key.mid(8)), value);
        else if (key.startsWith(QStringLiteral("device.")) && key.endsWith(QStringLiteral(".profile")))
            appendRule(QStringLiteral("device"), key.mid(7, key.size() - 15), value);
        else if (key.startsWith(QStringLiteral("device-type.")) && key.endsWith(QStringLiteral(".profile")))
            appendRule(QStringLiteral("device-type"), key.mid(12, key.size() - 20), value);
    }

    bool saveConfig()
    {
        QFileInfo info(configPath->text());
        QDir dir = info.dir();
        if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
            QMessageBox::warning(this, QStringLiteral("Cannot create directory"), dir.path());
            return false;
        }
        QFile file(info.filePath());
        if (!file.open(QIODevice::WriteOnly | QIODevice::Text | QIODevice::Truncate)) {
            QMessageBox::warning(this, QStringLiteral("Cannot write config"), file.errorString());
            return false;
        }
        QTextStream out(&file);
        out << "# Generated by wiiland-config.\n";
        out << "backend=uinput\n";
        out << "profile=" << profile->currentText() << "\n";
        out << "pointer-speed=" << pointerSpeed->value() << "\n";
        out << "ir-speed=" << irSpeed->value() << "\n";
        out << "ir-deadzone=" << irDeadzone->value() << "\n";
        out << "ir-smoothing=" << irSmoothing->value() << "\n";
        for (auto it = desktopActions.cbegin(); it != desktopActions.cend(); ++it)
            out << "desktop." << it.key() << '=' << it.value()->currentText() << "\n";
        for (int row = 0; row < rules->rowCount(); ++row) {
            auto *kindCombo = qobject_cast<QComboBox *>(rules->cellWidget(row, 0));
            auto *profileCombo = qobject_cast<QComboBox *>(rules->cellWidget(row, 2));
            auto *matchItem = rules->item(row, 1);
            if (!kindCombo || !profileCombo || !matchItem || matchItem->text().trimmed().isEmpty())
                continue;
            out << kindCombo->currentText() << '.' << matchItem->text().trimmed()
                << ".profile=" << profileCombo->currentText() << "\n";
        }
        statusBar()->showMessage(QStringLiteral("Saved %1").arg(info.filePath()), 4000);
        return true;
    }

    QLineEdit *wiilanddPath = nullptr;
    QLineEdit *configPath = nullptr;
    QLineEdit *deviceSelector = nullptr;
    QComboBox *traceFilter = nullptr;
    QComboBox *profile = nullptr;
    QSpinBox *pointerSpeed = nullptr;
    QSpinBox *irSpeed = nullptr;
    QSpinBox *irDeadzone = nullptr;
    QSpinBox *irSmoothing = nullptr;
    QHash<QString, QComboBox *> desktopActions;
    QTableWidget *rules = nullptr;
    QPlainTextEdit *output = nullptr;
    QProcess *traceProcess = nullptr;
};

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("wiiland-config"));
    QApplication::setOrganizationName(QStringLiteral("WiiLand"));
    MainWindow window;
    window.show();
    return app.exec();
}
