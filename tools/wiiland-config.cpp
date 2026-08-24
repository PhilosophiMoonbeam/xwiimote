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
#include <QtCore/QSaveFile>
#include <QtCore/QSharedPointer>
#include <QtCore/QStandardPaths>
#include <QtCore/QTextStream>
#include <QtCore/QTimer>
#include <QtCore/QStringList>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QTextCursor>
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
#include <QtWidgets/QScrollArea>
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

QStringList desktopBindingNames()
{
    return {
        QStringLiteral("a"),
        QStringLiteral("b"),
        QStringLiteral("plus"),
        QStringLiteral("minus"),
        QStringLiteral("home"),
        QStringLiteral("one"),
        QStringLiteral("two"),
    };
}

void setComboText(QComboBox *combo, const QString &value)
{
    if (!combo)
        return;
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
        tabs->setAccessibleName(QStringLiteral("WiiLand settings"));
        tabs->addTab(buildOverviewTab(tabs), QStringLiteral("Overview"));
        tabs->addTab(buildConfigTab(tabs), QStringLiteral("Configuration"));
        tabs->addTab(buildValidationTab(tabs), QStringLiteral("Validation"));
        rootLayout->addWidget(tabs, 3);

        auto *outputBox = new QGroupBox(QStringLiteral("Command output"), root);
        auto *outputLayout = new QVBoxLayout(outputBox);
        output = new QPlainTextEdit(outputBox);
        output->setAccessibleName(QStringLiteral("Command output"));
        output->setReadOnly(true);
        output->setLineWrapMode(QPlainTextEdit::NoWrap);
        outputLayout->addWidget(output);
        rootLayout->addWidget(outputBox, 2);

        setCentralWidget(root);
        const QString platformName = QGuiApplication::platformName();
        const QString displayBackend = platformName.startsWith(QStringLiteral("wayland"))
                                           ? QStringLiteral("native Wayland")
                                           : platformName;
        statusBar()->showMessage(
            QStringLiteral("Ready — display backend: %1").arg(displayBackend));
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
        wiilanddPath->setAccessibleName(QStringLiteral("wiilandd executable"));
        wiilanddPath->setPlaceholderText(QStringLiteral("wiilandd or an absolute path"));
        configPath->setAccessibleName(QStringLiteral("Configuration file"));
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

        layout->addStretch(1);

        return tab;
    }

    QWidget *buildConfigTab(QWidget *parent)
    {
        auto *tab = new QWidget(parent);
        auto *tabLayout = new QVBoxLayout(tab);
        auto *scroll = new QScrollArea(tab);
        auto *content = new QWidget(scroll);
        auto *layout = new QHBoxLayout(content);

        scroll->setWidgetResizable(true);
        scroll->setWidget(content);
        tabLayout->addWidget(scroll);
        auto *profileBox = new QGroupBox(QStringLiteral("Profiles and pointer feel"), tab);
        auto *profileForm = new QFormLayout(profileBox);
        profile = new QComboBox(profileBox);
        profile->addItems({QStringLiteral("gamepad"), QStringLiteral("desktop"), QStringLiteral("both")});
        pointerSpeed = spinBox(1, 127, 16, profileBox);
        irSpeed = spinBox(1, 127, 8, profileBox);
        irDeadzone = spinBox(0, 127, 0, profileBox);
        irSmoothing = spinBox(0, 95, 0, profileBox);
        irTracking = new QComboBox(profileBox);
        irTracking->addItems({QStringLiteral("dual"), QStringLiteral("centroid"), QStringLiteral("first")});
        irAimMapping = new QComboBox(profileBox);
        irAimMapping->addItems({QStringLiteral("relative"), QStringLiteral("absolute")});
        irScreenCalibrationEnabled = new QCheckBox(profileBox);
        irScreenLeft = spinBox(0, 32767, 0, profileBox);
        irScreenRight = spinBox(0, 32767, 1023, profileBox);
        irScreenTop = spinBox(0, 32767, 0, profileBox);
        irScreenBottom = spinBox(0, 32767, 767, profileBox);
        const auto syncIrScreenWidgets = [this](bool enabled) {
            irScreenLeft->setEnabled(enabled);
            irScreenRight->setEnabled(enabled);
            irScreenTop->setEnabled(enabled);
            irScreenBottom->setEnabled(enabled);
        };
        connect(irScreenCalibrationEnabled, &QCheckBox::toggled, this, syncIrScreenWidgets);
        syncIrScreenWidgets(false);
        profileForm->addRow(QStringLiteral("Default profile"), profile);
        profileForm->addRow(QStringLiteral("D-pad pointer speed"), pointerSpeed);
        profileForm->addRow(QStringLiteral("IR pointer gain"), irSpeed);
        profileForm->addRow(QStringLiteral("IR jitter deadzone"), irDeadzone);
        profileForm->addRow(QStringLiteral("IR smoothing %"), irSmoothing);
        profileForm->addRow(QStringLiteral("IR tracking"), irTracking);
        profileForm->addRow(QStringLiteral("IR aim mapping"), irAimMapping);
        profileForm->addRow(QStringLiteral("Use screen calibration"), irScreenCalibrationEnabled);
        profileForm->addRow(QStringLiteral("IR screen left"), irScreenLeft);
        profileForm->addRow(QStringLiteral("IR screen right"), irScreenRight);
        profileForm->addRow(QStringLiteral("IR screen top"), irScreenTop);
        profileForm->addRow(QStringLiteral("IR screen bottom"), irScreenBottom);

        auto *aimBox = new QGroupBox(QStringLiteral("Modern motion aiming"), tab);
        auto *aimForm = new QFormLayout(aimBox);
        aimMode = new QComboBox(aimBox);
        aimMode->addItems({QStringLiteral("off"), QStringLiteral("right-stick"), QStringLiteral("mouse")});
        aimSource = new QComboBox(aimBox);
        aimSource->addItems({QStringLiteral("auto"), QStringLiteral("ir"), QStringLiteral("motion-plus"), QStringLiteral("accelerometer")});
        aimActivation = new QComboBox(aimBox);
        aimActivation->addItems({QStringLiteral("b"), QStringLiteral("always"), QStringLiteral("z"), QStringLiteral("c")});
        aimSensitivity = spinBox(1, 127, 16, aimBox);
        aimDeadzone = spinBox(0, 32767, 4, aimBox);
        aimSmoothing = spinBox(0, 95, 25, aimBox);
        aimInvertX = new QCheckBox(aimBox);
        aimInvertY = new QCheckBox(aimBox);
        aimCalibrationEnabled = new QCheckBox(aimBox);
        aimCalibrationDuration = spinBox(1, 30, 8, aimBox);
        aimCalibrationDuration->setSuffix(QStringLiteral(" s"));
        aimAccelZeroX = spinBox(-32768, 32767, 0, aimBox);
        aimAccelZeroY = spinBox(-32768, 32767, 0, aimBox);
        aimAccelZeroZ = spinBox(-32768, 32767, 0, aimBox);
        aimMotionPlusBiasX = spinBox(-32768, 32767, 0, aimBox);
        aimMotionPlusBiasY = spinBox(-32768, 32767, 0, aimBox);
        aimMotionPlusBiasZ = spinBox(-32768, 32767, 0, aimBox);
        aimCalibrationEnabled->setToolTip(QStringLiteral("Save flat-surface accelerometer and MotionPlus offsets from --calibrate-aim."));
        const auto syncCalibrationWidgets = [this](bool enabled) {
            aimAccelZeroX->setEnabled(enabled);
            aimAccelZeroY->setEnabled(enabled);
            aimAccelZeroZ->setEnabled(enabled);
            aimMotionPlusBiasX->setEnabled(enabled);
            aimMotionPlusBiasY->setEnabled(enabled);
            aimMotionPlusBiasZ->setEnabled(enabled);
        };
        connect(aimCalibrationEnabled, &QCheckBox::toggled, this, syncCalibrationWidgets);
        syncCalibrationWidgets(false);
        aimForm->addRow(QStringLiteral("Output"), aimMode);
        aimForm->addRow(QStringLiteral("Best available sensor"), aimSource);
        aimForm->addRow(QStringLiteral("Activation"), aimActivation);
        aimForm->addRow(QStringLiteral("Sensitivity"), aimSensitivity);
        aimForm->addRow(QStringLiteral("Deadzone"), aimDeadzone);
        aimForm->addRow(QStringLiteral("Smoothing %"), aimSmoothing);
        aimForm->addRow(QStringLiteral("Invert X"), aimInvertX);
        aimForm->addRow(QStringLiteral("Invert Y"), aimInvertY);
        aimForm->addRow(QStringLiteral("Use saved calibration"), aimCalibrationEnabled);
        aimForm->addRow(QStringLiteral("Calibration duration"), aimCalibrationDuration);
        aimForm->addRow(QStringLiteral("Accelerometer zero X"), aimAccelZeroX);
        aimForm->addRow(QStringLiteral("Accelerometer zero Y"), aimAccelZeroY);
        aimForm->addRow(QStringLiteral("Accelerometer zero Z"), aimAccelZeroZ);
        aimForm->addRow(QStringLiteral("MotionPlus bias X"), aimMotionPlusBiasX);
        aimForm->addRow(QStringLiteral("MotionPlus bias Y"), aimMotionPlusBiasY);
        aimForm->addRow(QStringLiteral("MotionPlus bias Z"), aimMotionPlusBiasZ);

        auto *bindingsBox = new QGroupBox(QStringLiteral("Desktop button bindings"), tab);
        auto *bindingsForm = new QFormLayout(bindingsBox);
        for (const QString &name : desktopBindingNames()) {
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
        rules->setAccessibleName(QStringLiteral("Per-device profile rules"));
        rules->setSelectionBehavior(QAbstractItemView::SelectRows);
        rules->setSelectionMode(QAbstractItemView::SingleSelection);
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
        connect(removeRule, &QPushButton::clicked, this, [this]() {
            const int row = rules->currentRow();
            if (row >= 0)
                rules->removeRow(row);
        });

        auto *left = new QVBoxLayout;
        left->addWidget(profileBox);
        left->addWidget(aimBox);
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

        content->setMinimumSize(content->minimumSizeHint());
        return tab;
    }

    QWidget *buildValidationTab(QWidget *parent)
    {
        auto *tab = new QWidget(parent);
        auto *layout = new QVBoxLayout(tab);

        auto *matrix = new QGroupBox(QStringLiteral("Hardware validation capture"), tab);
        auto *matrixLayout = new QFormLayout(matrix);
        deviceSelector = new QLineEdit(matrix);
        deviceSelector->setAccessibleName(QStringLiteral("Device number or sysfs path"));
        deviceSelector->setPlaceholderText(QStringLiteral("1 or /sys/devices/..."));
        traceFilter = new QComboBox(matrix);
        traceFilter->addItems({QStringLiteral("all"), QStringLiteral("keys"), QStringLiteral("axes"), QStringLiteral("ir"), QStringLiteral("motion-plus")});
        matrixLayout->addRow(QStringLiteral("Device number or /sys path"), deviceSelector);
        matrixLayout->addRow(QStringLiteral("Trace filter"), traceFilter);
        layout->addWidget(matrix);

        auto *buttons = new QHBoxLayout;
        auto *startTraceButton = new QPushButton(QStringLiteral("Start dry-run trace"), tab);
        auto *stopTraceButton = new QPushButton(QStringLiteral("Stop trace"), tab);
        auto *clear = new QPushButton(QStringLiteral("Clear output"), tab);
        auto *calibrateButton = new QPushButton(QStringLiteral("Capture flat-surface calibration"), tab);
        buttons->addWidget(startTraceButton);
        buttons->addWidget(stopTraceButton);
        buttons->addWidget(clear);
        buttons->addWidget(calibrateButton);
        buttons->addStretch(1);
        layout->addLayout(buttons);
        connect(startTraceButton, &QPushButton::clicked, this, [this]() { startTrace(); });
        connect(stopTraceButton, &QPushButton::clicked, this, [this]() { stopTrace(); });
        connect(clear, &QPushButton::clicked, this, [this]() { output->clear(); });
        connect(calibrateButton, &QPushButton::clicked, this, [this]() { calibrateAim(); });

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
        if (!output || text.isEmpty())
            return;

        QTextCursor cursor = output->textCursor();
        cursor.movePosition(QTextCursor::End);
        cursor.insertText(text);
        output->setTextCursor(cursor);
        output->ensureCursorVisible();
    }

    void appendOutputLine(const QString &text)
    {
        appendOutput(text + QLatin1Char('\n'));
    }

    void runCommand(const QStringList &arguments)
    {
        auto *process = new QProcess(this);
        process->setProcessChannelMode(QProcess::MergedChannels);
        const QString program = wiilanddPath->text().trimmed().isEmpty()
            ? QStringLiteral("wiilandd")
            : wiilanddPath->text().trimmed();
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(program, arguments));
        connect(process, &QProcess::readyReadStandardOutput, this, [this, process]() {
            appendOutput(QString::fromLocal8Bit(process->readAllStandardOutput()));
        });
        connect(process, &QProcess::errorOccurred, this,
                [this, process](QProcess::ProcessError error) {
            appendOutputLine(QStringLiteral("process error: ") + process->errorString());
            if (error == QProcess::FailedToStart) {
                process->deleteLater();
                statusBar()->showMessage(QStringLiteral("Command failed to start"), 4000);
            }
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this, [this, process](int code, QProcess::ExitStatus status) {
            appendOutput(QString::fromLocal8Bit(process->readAllStandardOutput()));
            if (status != QProcess::NormalExit || code != 0)
                appendOutputLine(QStringLiteral("exit status: %1").arg(code));
            process->deleteLater();
            statusBar()->showMessage(QStringLiteral("Command finished"), 4000);
        });
        process->start(program, arguments);
        statusBar()->showMessage(QStringLiteral("Command running"));
    }

    void startTrace()
    {
        if (traceProcess) {
            statusBar()->showMessage(QStringLiteral("A trace is already running or stopping"), 4000);
            return;
        }
        auto *process = new QProcess(this);
        traceProcess = process;
        process->setProcessChannelMode(QProcess::MergedChannels);
        connect(process, &QProcess::readyReadStandardOutput, this, [this, process]() {
            appendOutput(QString::fromLocal8Bit(process->readAllStandardOutput()));
        });
        connect(process, &QProcess::errorOccurred, this,
                [this, process](QProcess::ProcessError error) {
            appendOutputLine(QStringLiteral("trace error: ") + process->errorString());
            if (error == QProcess::FailedToStart) {
                if (traceProcess == process)
                    traceProcess = nullptr;
                process->deleteLater();
                statusBar()->showMessage(QStringLiteral("Trace failed to start"), 4000);
            }
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this, [this, process](int code, QProcess::ExitStatus status) {
            appendOutput(QString::fromLocal8Bit(process->readAllStandardOutput()));
            appendOutputLine(QStringLiteral("trace stopped: exit=%1 status=%2")
                                 .arg(code)
                                 .arg(status));
            if (traceProcess == process)
                traceProcess = nullptr;
            process->deleteLater();
            statusBar()->showMessage(QStringLiteral("Trace stopped"), 4000);
        });

        QStringList args{QStringLiteral("--dry-run"), QStringLiteral("--trace-events=") + traceFilter->currentText(),
                         QStringLiteral("--verbose"), QStringLiteral("--profile"), QStringLiteral("both")};
        const QString device = deviceSelector->text().trimmed();
        if (!device.isEmpty())
            args << QStringLiteral("--device") << device;
        const QString program = wiilanddPath->text().trimmed().isEmpty()
            ? QStringLiteral("wiilandd")
            : wiilanddPath->text().trimmed();
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(program, args));
        process->start(program, args);
        statusBar()->showMessage(QStringLiteral("Trace running"));
    }

    void stopTrace()
    {
        QProcess *process = traceProcess;

        if (!process)
            return;
        process->terminate();
        QTimer::singleShot(1500, process, [process]() {
            if (process->state() != QProcess::NotRunning)
                process->kill();
        });
        statusBar()->showMessage(QStringLiteral("Stopping trace"));
    }

    void calibrateAim()
    {
        if (traceProcess) {
            QMessageBox::information(
                this,
                QStringLiteral("Trace is active"),
                QStringLiteral("Stop the dry-run trace before capturing calibration."));
            return;
        }

        QStringList args{
            QStringLiteral("--calibrate-aim"),
            QStringLiteral("--aim-calibration-duration"),
            QString::number(aimCalibrationDuration->value()),
        };
        const QString device = deviceSelector->text().trimmed();
        if (!device.isEmpty())
            args << QStringLiteral("--device") << device;

        auto *process = new QProcess(this);
        auto captured = QSharedPointer<QString>::create();
        process->setProcessChannelMode(QProcess::MergedChannels);
        const QString program = wiilanddPath->text().trimmed().isEmpty()
            ? QStringLiteral("wiilandd")
            : wiilanddPath->text().trimmed();
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(program, args));
        connect(process, &QProcess::readyReadStandardOutput, this, [this, process, captured]() {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardOutput());
            *captured += chunk;
            appendOutput(chunk);
        });
        connect(process, &QProcess::errorOccurred, this,
                [this, process](QProcess::ProcessError error) {
            appendOutputLine(QStringLiteral("process error: ") + process->errorString());
            if (error == QProcess::FailedToStart) {
                process->deleteLater();
                statusBar()->showMessage(QStringLiteral("Calibration failed to start"), 4000);
            }
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this, [this, process, captured](int code, QProcess::ExitStatus status) {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardOutput());
            *captured += chunk;
            appendOutput(chunk);
            if (status == QProcess::NormalExit && code == 0)
                applyCalibrationOutput(*captured);
            else
                appendOutputLine(QStringLiteral("exit status: %1").arg(code));
            process->deleteLater();
            statusBar()->showMessage(QStringLiteral("Calibration command finished"), 4000);
        });
        process->start(program, args);
        statusBar()->showMessage(QStringLiteral("Calibration running"));
    }

    void applyCalibrationOutput(const QString &text)
    {
        bool applied = false;

        const QStringList lines = text.split(QLatin1Char('\n'));
        for (const QString &rawLine : lines) {
            const QString line = rawLine.trimmed();
            if (line.isEmpty() || line.startsWith(QLatin1Char('#')))
                continue;
            const int equal = line.indexOf(QLatin1Char('='));
            if (equal <= 0)
                continue;
            const QString key = line.left(equal).trimmed();
            if (!key.startsWith(QStringLiteral("aim-accel-zero-")) &&
                !key.startsWith(QStringLiteral("aim-motion-plus-bias-")) &&
                key != QStringLiteral("aim-calibration-duration"))
                continue;
            applyConfigValue(key, line.mid(equal + 1).trimmed());
            applied = true;
        }

        if (applied)
            statusBar()->showMessage(QStringLiteral("Calibration values applied to the form; save the config to persist them"), 6000);
        else
            appendOutputLine(QStringLiteral("No calibration key=value lines were captured."));
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

    void resetConfigForm()
    {
        setComboText(profile, QStringLiteral("gamepad"));
        pointerSpeed->setValue(16);
        irSpeed->setValue(8);
        irDeadzone->setValue(0);
        irSmoothing->setValue(0);
        setComboText(irTracking, QStringLiteral("dual"));
        setComboText(irAimMapping, QStringLiteral("relative"));
        irScreenCalibrationEnabled->setChecked(false);
        irScreenLeft->setValue(0);
        irScreenRight->setValue(1023);
        irScreenTop->setValue(0);
        irScreenBottom->setValue(767);
        setComboText(aimMode, QStringLiteral("off"));
        setComboText(aimSource, QStringLiteral("auto"));
        setComboText(aimActivation, QStringLiteral("b"));
        aimSensitivity->setValue(16);
        aimDeadzone->setValue(4);
        aimSmoothing->setValue(25);
        aimInvertX->setChecked(false);
        aimInvertY->setChecked(false);
        aimCalibrationEnabled->setChecked(false);
        aimCalibrationDuration->setValue(8);
        aimAccelZeroX->setValue(0);
        aimAccelZeroY->setValue(0);
        aimAccelZeroZ->setValue(0);
        aimMotionPlusBiasX->setValue(0);
        aimMotionPlusBiasY->setValue(0);
        aimMotionPlusBiasZ->setValue(0);
        setComboText(desktopActions.value(QStringLiteral("a")), QStringLiteral("left-click"));
        setComboText(desktopActions.value(QStringLiteral("b")), QStringLiteral("right-click"));
        setComboText(desktopActions.value(QStringLiteral("plus")), QStringLiteral("enter"));
        setComboText(desktopActions.value(QStringLiteral("minus")), QStringLiteral("escape"));
        setComboText(desktopActions.value(QStringLiteral("home")), QStringLiteral("overview"));
        setComboText(desktopActions.value(QStringLiteral("one")), QStringLiteral("page-down"));
        setComboText(desktopActions.value(QStringLiteral("two")), QStringLiteral("page-up"));
        rules->setRowCount(0);
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

        resetConfigForm();
        QTextStream in(&file);
        while (!in.atEnd()) {
            QString line = in.readLine();
            const int comment = line.indexOf(QLatin1Char('#'));
            if (comment >= 0)
                line.truncate(comment);
            line = line.trimmed();
            if (line.isEmpty())
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
        else if (key == QStringLiteral("ir-tracking"))
            setComboText(irTracking, value);
        else if (key == QStringLiteral("ir-aim-mapping"))
            setComboText(irAimMapping, value);
        else if (key == QStringLiteral("ir-screen-left")) {
            irScreenCalibrationEnabled->setChecked(true);
            irScreenLeft->setValue(value.toInt());
        } else if (key == QStringLiteral("ir-screen-right")) {
            irScreenCalibrationEnabled->setChecked(true);
            irScreenRight->setValue(value.toInt());
        } else if (key == QStringLiteral("ir-screen-top")) {
            irScreenCalibrationEnabled->setChecked(true);
            irScreenTop->setValue(value.toInt());
        } else if (key == QStringLiteral("ir-screen-bottom")) {
            irScreenCalibrationEnabled->setChecked(true);
            irScreenBottom->setValue(value.toInt());
        }
        else if (key == QStringLiteral("aim-mode"))
            setComboText(aimMode, value);
        else if (key == QStringLiteral("aim-source"))
            setComboText(aimSource, value);
        else if (key == QStringLiteral("aim-activation"))
            setComboText(aimActivation, value);
        else if (key == QStringLiteral("aim-sensitivity"))
            aimSensitivity->setValue(value.toInt());
        else if (key == QStringLiteral("aim-deadzone"))
            aimDeadzone->setValue(value.toInt());
        else if (key == QStringLiteral("aim-smoothing"))
            aimSmoothing->setValue(value.toInt());
        else if (key == QStringLiteral("aim-invert-x"))
            aimInvertX->setChecked(value == QStringLiteral("yes") || value == QStringLiteral("true") || value == QStringLiteral("1"));
        else if (key == QStringLiteral("aim-accel-zero-x")) {
            aimCalibrationEnabled->setChecked(true);
            aimAccelZeroX->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-accel-zero-y")) {
            aimCalibrationEnabled->setChecked(true);
            aimAccelZeroY->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-accel-zero-z")) {
            aimCalibrationEnabled->setChecked(true);
            aimAccelZeroZ->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-motion-plus-bias-x")) {
            aimCalibrationEnabled->setChecked(true);
            aimMotionPlusBiasX->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-motion-plus-bias-y")) {
            aimCalibrationEnabled->setChecked(true);
            aimMotionPlusBiasY->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-motion-plus-bias-z")) {
            aimCalibrationEnabled->setChecked(true);
            aimMotionPlusBiasZ->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-calibration-duration"))
            aimCalibrationDuration->setValue(value.toInt());
        else if (key == QStringLiteral("aim-invert-y"))
            aimInvertY->setChecked(value == QStringLiteral("yes") || value == QStringLiteral("true") || value == QStringLiteral("1"));
        else if (key.startsWith(QStringLiteral("desktop.")))
            setComboText(desktopActions.value(key.mid(8)), value);
        else if (key.startsWith(QStringLiteral("device.")) && key.endsWith(QStringLiteral(".profile")))
            appendRule(QStringLiteral("device"), key.mid(7, key.size() - 15), value);
        else if (key.startsWith(QStringLiteral("device-type.")) && key.endsWith(QStringLiteral(".profile")))
            appendRule(QStringLiteral("device-type"), key.mid(12, key.size() - 20), value);
    }

    bool saveConfig()
    {
        if (irScreenCalibrationEnabled->isChecked() &&
            (irScreenRight->value() <= irScreenLeft->value() ||
             irScreenBottom->value() <= irScreenTop->value())) {
            QMessageBox::warning(
                this,
                QStringLiteral("Invalid IR screen calibration"),
                QStringLiteral("IR screen right must exceed left, and bottom must exceed top."));
            return false;
        }

        for (int row = 0; row < rules->rowCount(); ++row) {
            auto *matchItem = rules->item(row, 1);
            if (!matchItem)
                continue;
            const QString match = matchItem->text().trimmed();
            if (match.contains(QLatin1Char('#')) || match.contains(QLatin1Char('=')) ||
                match.contains(QLatin1Char('\n')) || match.contains(QLatin1Char('\r'))) {
                QMessageBox::warning(
                    this,
                    QStringLiteral("Invalid device rule"),
                    QStringLiteral("Rule match text cannot contain #, =, or line breaks."));
                return false;
            }
        }

        QFileInfo info(configPath->text());
        QDir dir = info.dir();
        if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
            QMessageBox::warning(this, QStringLiteral("Cannot create directory"), dir.path());
            return false;
        }
        QSaveFile file(info.filePath());
        if (!file.open(QIODevice::WriteOnly | QIODevice::Text)) {
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
        out << "ir-tracking=" << irTracking->currentText() << "\n";
        out << "ir-aim-mapping=" << irAimMapping->currentText() << "\n";
        if (irScreenCalibrationEnabled->isChecked()) {
            out << "ir-screen-left=" << irScreenLeft->value() << "\n";
            out << "ir-screen-right=" << irScreenRight->value() << "\n";
            out << "ir-screen-top=" << irScreenTop->value() << "\n";
            out << "ir-screen-bottom=" << irScreenBottom->value() << "\n";
        }
        out << "aim-mode=" << aimMode->currentText() << "\n";
        out << "aim-source=" << aimSource->currentText() << "\n";
        out << "aim-activation=" << aimActivation->currentText() << "\n";
        out << "aim-sensitivity=" << aimSensitivity->value() << "\n";
        out << "aim-deadzone=" << aimDeadzone->value() << "\n";
        out << "aim-smoothing=" << aimSmoothing->value() << "\n";
        out << "aim-invert-x=" << (aimInvertX->isChecked() ? "yes" : "no") << "\n";
        if (aimCalibrationEnabled->isChecked()) {
            out << "aim-accel-zero-x=" << aimAccelZeroX->value() << "\n";
            out << "aim-accel-zero-y=" << aimAccelZeroY->value() << "\n";
            out << "aim-accel-zero-z=" << aimAccelZeroZ->value() << "\n";
            out << "aim-motion-plus-bias-x=" << aimMotionPlusBiasX->value() << "\n";
            out << "aim-motion-plus-bias-y=" << aimMotionPlusBiasY->value() << "\n";
            out << "aim-motion-plus-bias-z=" << aimMotionPlusBiasZ->value() << "\n";
        }
        out << "aim-calibration-duration=" << aimCalibrationDuration->value() << "\n";
        out << "aim-invert-y=" << (aimInvertY->isChecked() ? "yes" : "no") << "\n";
        for (const QString &name : desktopBindingNames())
            out << "desktop." << name << '=' << desktopActions.value(name)->currentText() << "\n";
        for (int row = 0; row < rules->rowCount(); ++row) {
            auto *kindCombo = qobject_cast<QComboBox *>(rules->cellWidget(row, 0));
            auto *profileCombo = qobject_cast<QComboBox *>(rules->cellWidget(row, 2));
            auto *matchItem = rules->item(row, 1);
            if (!kindCombo || !profileCombo || !matchItem || matchItem->text().trimmed().isEmpty())
                continue;
            out << kindCombo->currentText() << '.' << matchItem->text().trimmed()
                << ".profile=" << profileCombo->currentText() << "\n";
        }
        out.flush();
        if (out.status() != QTextStream::Ok) {
            file.cancelWriting();
            QMessageBox::warning(this, QStringLiteral("Cannot write config"), file.errorString());
            return false;
        }
        if (!file.commit()) {
            QMessageBox::warning(this, QStringLiteral("Cannot replace config"), file.errorString());
            return false;
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
    QComboBox *aimMode = nullptr;
    QComboBox *irTracking = nullptr;
    QComboBox *irAimMapping = nullptr;
    QCheckBox *irScreenCalibrationEnabled = nullptr;
    QSpinBox *irScreenLeft = nullptr;
    QSpinBox *irScreenRight = nullptr;
    QSpinBox *irScreenTop = nullptr;
    QSpinBox *irScreenBottom = nullptr;
    QComboBox *aimSource = nullptr;
    QComboBox *aimActivation = nullptr;
    QSpinBox *aimSensitivity = nullptr;
    QSpinBox *aimDeadzone = nullptr;
    QSpinBox *aimSmoothing = nullptr;
    QCheckBox *aimInvertX = nullptr;
    QCheckBox *aimCalibrationEnabled = nullptr;
    QSpinBox *aimCalibrationDuration = nullptr;
    QSpinBox *aimAccelZeroX = nullptr;
    QSpinBox *aimAccelZeroY = nullptr;
    QSpinBox *aimAccelZeroZ = nullptr;
    QSpinBox *aimMotionPlusBiasX = nullptr;
    QSpinBox *aimMotionPlusBiasY = nullptr;
    QSpinBox *aimMotionPlusBiasZ = nullptr;
    QCheckBox *aimInvertY = nullptr;
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
