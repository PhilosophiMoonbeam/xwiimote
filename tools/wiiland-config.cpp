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
#include <QtCore/QTextStream>
#include <QtCore/QTemporaryFile>
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
#include <QtWidgets/QSplitter>
#include <QtWidgets/QTableWidget>
#include <QtWidgets/QScrollArea>
#include <QtWidgets/QTableWidgetItem>
#include <QtWidgets/QTabWidget>
#include <QtWidgets/QVBoxLayout>
#include <QtWidgets/QWidget>

namespace {
constexpr int CommandOutputBlockLimit = 10000;


QString defaultConfigPath()
{
    const QString configHome = qEnvironmentVariable("XDG_CONFIG_HOME");
    if (QDir::isAbsolutePath(configHome))
        return configHome + QStringLiteral("/wiiland/wiilandd.conf");

    const QString home = qEnvironmentVariable("HOME");
    if (QDir::isAbsolutePath(home))
        return home + QStringLiteral("/.config/wiiland/wiilandd.conf");
    return {};
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

QString displayBackendName()
{
    const QString platformName = QGuiApplication::platformName();
    if (platformName.startsWith(QStringLiteral("wayland")))
        return QStringLiteral("Wayland");
    if (platformName == QStringLiteral("xcb"))
        return QStringLiteral("X11");
    return platformName;
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

        auto *title = new QLabel(QStringLiteral("WiiLand Control Center"), root);
        QFont titleFont = title->font();
        titleFont.setPointSize(titleFont.pointSize() + 4);
        titleFont.setBold(true);
        title->setFont(titleFont);
        rootLayout->addWidget(title);

        auto *subtitle = new QLabel(
            QStringLiteral("Configure Wii Remote input, inspect daemon readiness, and capture validation output."),
            root);
        subtitle->setWordWrap(true);
        rootLayout->addWidget(subtitle);

        auto *workspace = new QSplitter(Qt::Vertical, root);
        workspace->setAccessibleName(QStringLiteral("Settings and command output"));
        workspace->setChildrenCollapsible(false);

        auto *tabs = new QTabWidget;
        tabs->setAccessibleName(QStringLiteral("WiiLand settings"));
        tabs->addTab(buildOverviewTab(tabs), QStringLiteral("Overview"));
        tabs->addTab(buildConfigTab(tabs), QStringLiteral("Configuration"));
        tabs->addTab(buildValidationTab(tabs), QStringLiteral("Validation"));
        workspace->addWidget(tabs);

        auto *outputBox = new QGroupBox(QStringLiteral("Command output"));
        auto *outputLayout = new QVBoxLayout(outputBox);
        output = new QPlainTextEdit(outputBox);
        output->setAccessibleName(QStringLiteral("Command output"));
        output->setReadOnly(true);
        output->setLineWrapMode(QPlainTextEdit::NoWrap);
        output->setMaximumBlockCount(CommandOutputBlockLimit);
        outputLayout->addWidget(output);
        workspace->addWidget(outputBox);
        workspace->setStretchFactor(0, 3);
        workspace->setStretchFactor(1, 1);
        workspace->setSizes({480, 180});
        rootLayout->addWidget(workspace, 1);

        setCentralWidget(root);
        const QString backend = displayBackendName();
        statusBar()->showMessage(QStringLiteral("Ready — Qt display backend: %1").arg(backend));
        loadConfigFromPath(defaultConfigPath(), false);
        refreshServiceStatus();
    }

    bool writeSmokeReport(QTextStream &stream)
    {
        const QString defaultPath = defaultConfigPath();
        const bool defaultPathAbsolute = QDir::isAbsolutePath(defaultPath);
        configPath->setText(defaultPath + QStringLiteral(".explicit-smoke"));
        const bool explicitRestartDisabled = !saveAndRestartButton->isEnabled();

        resetConfigForm();
        aimAccelCalibrationEnabled->setChecked(true);
        aimAccelZeroX->setValue(11);
        aimAccelZeroY->setValue(12);
        aimAccelZeroZ->setValue(13);
        const QByteArray config = renderedConfig();
        const bool calibrationSourcesIsolated =
            config.contains("aim-accel-zero-x=11\n") &&
            config.contains("aim-accel-zero-y=12\n") &&
            config.contains("aim-accel-zero-z=13\n") &&
            !config.contains("aim-motion-plus-bias-");
        const bool outputBounded =
            output->maximumBlockCount() == CommandOutputBlockLimit;

        stream << QStringLiteral("qt.platform=%1\n").arg(QGuiApplication::platformName())
               << QStringLiteral("service.restart.explicit-config=%1\n")
                      .arg(explicitRestartDisabled
                               ? QStringLiteral("disabled")
                               : QStringLiteral("enabled"))
               << QStringLiteral("calibration.partial-source=%1\n")
                      .arg(calibrationSourcesIsolated
                               ? QStringLiteral("isolated")
                               : QStringLiteral("coupled"))
               << QStringLiteral("config.default-path=%1\n")
                      .arg(defaultPathAbsolute
                               ? QStringLiteral("absolute")
                               : QStringLiteral("invalid"))
               << QStringLiteral("output.buffer=%1\n")
                      .arg(outputBounded
                               ? QStringLiteral("bounded")
                               : QStringLiteral("unbounded"));
        return defaultPathAbsolute && explicitRestartDisabled &&
               calibrationSourcesIsolated && outputBounded;
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
        configPath->setObjectName(QStringLiteral("configPath"));
        wiilanddPath->setAccessibleName(QStringLiteral("wiilandd executable"));
        wiilanddPath->setPlaceholderText(QStringLiteral("wiilandd or an absolute path"));
        configPath->setAccessibleName(QStringLiteral("Configuration file"));
        auto *browse = new QPushButton(QStringLiteral("Browse..."), paths);
        auto *configRow = new QWidget(paths);
        auto *configRowLayout = new QHBoxLayout(configRow);
        configRowLayout->setContentsMargins(0, 0, 0, 0);
        configRowLayout->addWidget(configPath, 1);
        configRowLayout->addWidget(browse);
        configScope = new QLabel(paths);
        configScope->setWordWrap(true);
        configScope->setAccessibleName(QStringLiteral("Configuration command scope"));
        auto *backend = new QLabel(displayBackendName(), paths);
        backend->setAccessibleName(QStringLiteral("Qt display backend"));
        form->addRow(QStringLiteral("wiilandd executable"), wiilanddPath);
        form->addRow(QStringLiteral("Config file"), configRow);
        form->addRow(QStringLiteral("Command scope"), configScope);
        form->addRow(QStringLiteral("Qt display backend"), backend);
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
        connect(configPath, &QLineEdit::textChanged, this, [this]() { updateConfigScope(); });
        updateConfigScope();

        auto *service = new QGroupBox(QStringLiteral("Daemon service"), tab);
        auto *serviceLayout = new QHBoxLayout(service);
        serviceStatus = new QLabel(QStringLiteral("Checking..."), service);
        serviceStatus->setAccessibleName(QStringLiteral("wiilandd service status"));
        serviceLayout->addWidget(serviceStatus, 1);
        serviceRefresh = new QPushButton(QStringLiteral("Refresh"), service);
        serviceStart = new QPushButton(QStringLiteral("Start"), service);
        serviceStop = new QPushButton(QStringLiteral("Stop"), service);
        serviceRestart = new QPushButton(QStringLiteral("Restart"), service);
        serviceLayout->addWidget(serviceRefresh);
        serviceLayout->addWidget(serviceStart);
        serviceLayout->addWidget(serviceStop);
        serviceLayout->addWidget(serviceRestart);
        connect(serviceRefresh, &QPushButton::clicked, this, [this]() { refreshServiceStatus(); });
        connect(serviceStart, &QPushButton::clicked, this, [this]() { runServiceAction(QStringLiteral("start")); });
        connect(serviceStop, &QPushButton::clicked, this, [this]() { runServiceAction(QStringLiteral("stop")); });
        connect(serviceRestart, &QPushButton::clicked, this, [this]() { runServiceAction(QStringLiteral("restart")); });
        layout->addWidget(service);

        auto *quick = new QGroupBox(QStringLiteral("Readiness checks"), tab);
        auto *quickLayout = new QGridLayout(quick);
        const auto addButton = [this, quickLayout](
                                   const QString &text,
                                   const QStringList &args,
                                   bool configSensitive,
                                   int row,
                                   int column) {
            auto *button = new QPushButton(text);
            quickLayout->addWidget(button, row, column);
            connect(button, &QPushButton::clicked, this, [this, args, configSensitive]() {
                runCommand(args, configSensitive);
            });
        };
        addButton(QStringLiteral("Doctor"), {QStringLiteral("--doctor")}, true, 0, 0);
        addButton(QStringLiteral("Check config"), {QStringLiteral("--check-config")}, true, 0, 1);
        addButton(QStringLiteral("Dump config"), {QStringLiteral("--dump-config")}, true, 0, 2);
        addButton(QStringLiteral("List devices"), {QStringLiteral("--list"), QStringLiteral("--verbose")}, false, 1, 0);
        addButton(QStringLiteral("Axis/button map"), {QStringLiteral("--axis-map")}, false, 1, 1);
        addButton(QStringLiteral("Validation checklist"), {QStringLiteral("--validation-checklist")}, false, 1, 2);
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
        aimAccelCalibrationEnabled = new QCheckBox(aimBox);
        aimMotionPlusCalibrationEnabled = new QCheckBox(aimBox);
        aimCalibrationDuration = spinBox(1, 30, 8, aimBox);
        aimCalibrationDuration->setSuffix(QStringLiteral(" s"));
        aimAccelZeroX = spinBox(-32768, 32767, 0, aimBox);
        aimAccelZeroY = spinBox(-32768, 32767, 0, aimBox);
        aimAccelZeroZ = spinBox(-32768, 32767, 0, aimBox);
        aimMotionPlusBiasX = spinBox(-32768, 32767, 0, aimBox);
        aimMotionPlusBiasY = spinBox(-32768, 32767, 0, aimBox);
        aimMotionPlusBiasZ = spinBox(-32768, 32767, 0, aimBox);
        aimAccelCalibrationEnabled->setToolTip(
            QStringLiteral("Save the complete accelerometer zero point from --calibrate-aim."));
        aimMotionPlusCalibrationEnabled->setToolTip(
            QStringLiteral("Save the complete MotionPlus bias from --calibrate-aim."));
        const auto syncAccelCalibrationWidgets = [this](bool enabled) {
            aimAccelZeroX->setEnabled(enabled);
            aimAccelZeroY->setEnabled(enabled);
            aimAccelZeroZ->setEnabled(enabled);
        };
        const auto syncMotionPlusCalibrationWidgets = [this](bool enabled) {
            aimMotionPlusBiasX->setEnabled(enabled);
            aimMotionPlusBiasY->setEnabled(enabled);
            aimMotionPlusBiasZ->setEnabled(enabled);
        };
        connect(aimAccelCalibrationEnabled,
                &QCheckBox::toggled,
                this,
                syncAccelCalibrationWidgets);
        connect(aimMotionPlusCalibrationEnabled,
                &QCheckBox::toggled,
                this,
                syncMotionPlusCalibrationWidgets);
        syncAccelCalibrationWidgets(false);
        syncMotionPlusCalibrationWidgets(false);
        aimForm->addRow(QStringLiteral("Output"), aimMode);
        aimForm->addRow(QStringLiteral("Best available sensor"), aimSource);
        aimForm->addRow(QStringLiteral("Activation"), aimActivation);
        aimForm->addRow(QStringLiteral("Sensitivity"), aimSensitivity);
        aimForm->addRow(QStringLiteral("Deadzone"), aimDeadzone);
        aimForm->addRow(QStringLiteral("Smoothing %"), aimSmoothing);
        aimForm->addRow(QStringLiteral("Invert X"), aimInvertX);
        aimForm->addRow(QStringLiteral("Invert Y"), aimInvertY);
        aimForm->addRow(QStringLiteral("Use accelerometer calibration"), aimAccelCalibrationEnabled);
        aimForm->addRow(QStringLiteral("Use MotionPlus calibration"), aimMotionPlusCalibrationEnabled);
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
        loadButton = new QPushButton(QStringLiteral("Load effective config"), tab);
        saveButton = new QPushButton(QStringLiteral("Validate and save"), tab);
        saveAndRestartButton = new QPushButton(QStringLiteral("Save and restart daemon"), tab);
        saveAndRestartButton->setObjectName(QStringLiteral("saveAndRestartButton"));
        actions->addWidget(loadButton);
        actions->addWidget(saveButton);
        actions->addWidget(saveAndRestartButton);
        actions->addStretch(1);
        layout->addLayout(actions);
        connect(loadButton, &QPushButton::clicked, this, [this]() {
            loadConfigFromPath(configPath->text(), true);
        });
        connect(saveButton, &QPushButton::clicked, this, [this]() { saveConfig(false); });
        connect(saveAndRestartButton, &QPushButton::clicked, this, [this]() { saveConfig(true); });

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
        traceProfile = new QComboBox(matrix);
        traceProfile->addItem(QStringLiteral("Use effective configuration"), QString());
        traceProfile->addItem(QStringLiteral("Temporarily use gamepad"), QStringLiteral("gamepad"));
        traceProfile->addItem(QStringLiteral("Temporarily use desktop"), QStringLiteral("desktop"));
        traceProfile->addItem(QStringLiteral("Temporarily use both"), QStringLiteral("both"));
        traceProfile->setAccessibleName(QStringLiteral("Temporary trace profile override"));
        matrixLayout->addRow(QStringLiteral("Device number or /sys path"), deviceSelector);
        matrixLayout->addRow(QStringLiteral("Trace filter"), traceFilter);
        matrixLayout->addRow(QStringLiteral("Trace profile"), traceProfile);

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
            QStringLiteral("Suggested coverage: original Wii Remote, external and built-in MotionPlus, "
                           "Nunchuk, Classic Controller, Wii U Pro Controller, Guitar, Drums, Balance Board, "
                           "SDL, Wine/Proton, and desktop-profile behavior on Wayland and X11."),
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

    QString daemonProgram() const
    {
        const QString program = wiilanddPath->text().trimmed();
        return program.isEmpty() ? QStringLiteral("wiilandd") : program;
    }

    bool isExplicitConfigPath(const QString &path) const
    {
        const QString selected = path.trimmed();
        if (selected.isEmpty())
            return false;
        return QDir::cleanPath(QFileInfo(selected).absoluteFilePath()) !=
               QDir::cleanPath(QFileInfo(defaultConfigPath()).absoluteFilePath());
    }

    QStringList configuredArguments(QStringList arguments) const
    {
        const QString selected = configPath->text().trimmed();
        if (isExplicitConfigPath(selected))
            arguments = QStringList{QStringLiteral("--config"), selected} + arguments;
        return arguments;
    }

    void updateConfigScope()
    {
        const bool explicitPath = isExplicitConfigPath(configPath->text());
        if (explicitPath) {
            configScope->setText(
                QStringLiteral("Selected file only — config-sensitive commands use --config; "
                               "the installed user service does not load this path."));
        } else {
            configScope->setText(
                QStringLiteral("Effective layered configuration — daemon defaults, system file, then user file."));
        }

        if (saveAndRestartButton) {
            saveAndRestartButton->setEnabled(!savingConfig && !explicitPath);
            saveAndRestartButton->setToolTip(
                explicitPath
                    ? QStringLiteral("The installed user service only loads the default layered configuration.")
                    : QString());
        }
    }

    void setServiceControlsEnabled(bool refresh, bool start, bool stop, bool restart)
    {
        serviceRefresh->setEnabled(refresh);
        serviceStart->setEnabled(start);
        serviceStop->setEnabled(stop);
        serviceRestart->setEnabled(restart);
    }

    void refreshServiceStatus()
    {
        if (serviceProcess)
            return;

        serviceStatus->setText(QStringLiteral("Checking..."));
        setServiceControlsEnabled(false, false, false, false);
        auto *process = new QProcess(this);
        serviceProcess = process;
        const QStringList arguments{
            QStringLiteral("--user"),
            QStringLiteral("is-active"),
            QStringLiteral("wiilandd.service"),
        };
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(QStringLiteral("systemctl"), arguments));
        connect(process, &QProcess::errorOccurred, this,
                [this, process](QProcess::ProcessError error) {
            if (error != QProcess::FailedToStart)
                return;
            const QString detail = process->errorString();
            appendOutputLine(QStringLiteral("service query unavailable: ") + detail);
            serviceStatus->setText(QStringLiteral("Unavailable — %1").arg(detail));
            setServiceControlsEnabled(true, false, false, false);
            if (serviceProcess == process)
                serviceProcess = nullptr;
            process->deleteLater();
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this, [this, process](int code, QProcess::ExitStatus exitStatus) {
            const QString standardOutput =
                QString::fromLocal8Bit(process->readAllStandardOutput()).trimmed();
            const QString standardError =
                QString::fromLocal8Bit(process->readAllStandardError()).trimmed();
            if (!standardOutput.isEmpty())
                appendOutputLine(standardOutput);
            if (!standardError.isEmpty())
                appendOutputLine(standardError);

            if (exitStatus != QProcess::NormalExit) {
                serviceStatus->setText(QStringLiteral("Unavailable — systemctl crashed"));
                setServiceControlsEnabled(true, false, false, false);
            } else if (standardOutput == QStringLiteral("active") && code == 0) {
                serviceStatus->setText(QStringLiteral("Active"));
                setServiceControlsEnabled(true, false, true, true);
            } else if (standardOutput == QStringLiteral("inactive")) {
                serviceStatus->setText(QStringLiteral("Inactive"));
                setServiceControlsEnabled(true, true, false, true);
            } else if (standardOutput == QStringLiteral("failed")) {
                serviceStatus->setText(QStringLiteral("Failed"));
                setServiceControlsEnabled(true, true, false, true);
            } else if (standardOutput == QStringLiteral("activating") ||
                       standardOutput == QStringLiteral("deactivating") ||
                       standardOutput == QStringLiteral("reloading")) {
                QString state = standardOutput;
                state[0] = state.at(0).toUpper();
                serviceStatus->setText(state);
                setServiceControlsEnabled(true, false, false, false);
            } else {
                const QString detail = !standardError.isEmpty()
                    ? standardError
                    : (standardOutput.isEmpty()
                           ? QStringLiteral("systemctl exited with code %1").arg(code)
                           : standardOutput);
                serviceStatus->setText(QStringLiteral("Unavailable — %1").arg(detail));
                setServiceControlsEnabled(true, false, false, false);
            }

            if (serviceProcess == process)
                serviceProcess = nullptr;
            process->deleteLater();
        });
        process->start(QStringLiteral("systemctl"), arguments);
    }

    void runServiceAction(const QString &action)
    {
        if (serviceProcess)
            return;

        serviceStatus->setText(QStringLiteral("%1...").arg(
            action == QStringLiteral("start") ? QStringLiteral("Starting") :
            action == QStringLiteral("stop") ? QStringLiteral("Stopping") :
                                               QStringLiteral("Restarting")));
        setServiceControlsEnabled(false, false, false, false);
        auto *process = new QProcess(this);
        serviceProcess = process;
        const QStringList arguments{
            QStringLiteral("--user"),
            action,
            QStringLiteral("wiilandd.service"),
        };
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(QStringLiteral("systemctl"), arguments));
        connect(process, &QProcess::errorOccurred, this,
                [this, process, action](QProcess::ProcessError error) {
            if (error != QProcess::FailedToStart)
                return;
            const QString detail = process->errorString();
            appendOutputLine(QStringLiteral("service action unavailable: ") + detail);
            serviceStatus->setText(QStringLiteral("Unavailable — %1").arg(detail));
            setServiceControlsEnabled(true, false, false, false);
            QMessageBox::warning(
                this,
                QStringLiteral("Cannot %1 daemon service").arg(action),
                detail);
            if (serviceProcess == process)
                serviceProcess = nullptr;
            process->deleteLater();
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this, [this, process, action](int code, QProcess::ExitStatus exitStatus) {
            const QString standardOutput =
                QString::fromLocal8Bit(process->readAllStandardOutput()).trimmed();
            const QString standardError =
                QString::fromLocal8Bit(process->readAllStandardError()).trimmed();
            if (!standardOutput.isEmpty())
                appendOutputLine(standardOutput);
            if (!standardError.isEmpty())
                appendOutputLine(standardError);

            const bool succeeded = exitStatus == QProcess::NormalExit && code == 0;
            if (!succeeded) {
                const QString outcome = exitStatus == QProcess::NormalExit
                    ? QStringLiteral("systemctl exited with code %1.").arg(code)
                    : QStringLiteral("systemctl crashed.");
                const QString detail = !standardError.isEmpty() ? standardError : standardOutput;
                appendOutputLine(QStringLiteral("service action failed: ") + outcome);
                QMessageBox::warning(
                    this,
                    QStringLiteral("Cannot %1 daemon service").arg(action),
                    detail.isEmpty() ? outcome : outcome + QStringLiteral("\n\n") + detail);
            }

            if (serviceProcess == process)
                serviceProcess = nullptr;
            process->deleteLater();
            refreshServiceStatus();
        });
        process->start(QStringLiteral("systemctl"), arguments);
    }

    void runCommand(const QStringList &requestedArguments, bool configSensitive = false)
    {
        auto *process = new QProcess(this);
        process->setProcessChannelMode(QProcess::MergedChannels);
        const QStringList arguments =
            configSensitive ? configuredArguments(requestedArguments) : requestedArguments;
        const QString program = daemonProgram();
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
                this, [this, process](int code, QProcess::ExitStatus exitStatus) {
            appendOutput(QString::fromLocal8Bit(process->readAllStandardOutput()));
            if (exitStatus != QProcess::NormalExit) {
                appendOutputLine(QStringLiteral("command crashed"));
                statusBar()->showMessage(QStringLiteral("Command crashed"), 4000);
            } else if (code != 0) {
                appendOutputLine(QStringLiteral("exit status: %1").arg(code));
                statusBar()->showMessage(QStringLiteral("Command failed (exit %1)").arg(code), 4000);
            } else {
                statusBar()->showMessage(QStringLiteral("Command succeeded"), 4000);
            }
            process->deleteLater();
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
                this, [this, process](int code, QProcess::ExitStatus exitStatus) {
            appendOutput(QString::fromLocal8Bit(process->readAllStandardOutput()));
            if (exitStatus != QProcess::NormalExit)
                appendOutputLine(QStringLiteral("trace crashed"));
            else
                appendOutputLine(QStringLiteral("trace stopped: exit=%1").arg(code));
            if (traceProcess == process)
                traceProcess = nullptr;
            process->deleteLater();
            statusBar()->showMessage(
                exitStatus == QProcess::NormalExit && code == 0
                    ? QStringLiteral("Trace stopped")
                    : QStringLiteral("Trace failed"),
                4000);
        });

        QStringList args{
            QStringLiteral("--dry-run"),
            QStringLiteral("--trace-events=") + traceFilter->currentText(),
            QStringLiteral("--verbose"),
        };
        const QString temporaryProfile = traceProfile->currentData().toString();
        if (!temporaryProfile.isEmpty())
            args << QStringLiteral("--profile") << temporaryProfile;
        const QString device = deviceSelector->text().trimmed();
        if (!device.isEmpty())
            args << QStringLiteral("--device") << device;
        args = configuredArguments(args);
        const QString program = daemonProgram();
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
        args = configuredArguments(args);

        auto *process = new QProcess(this);
        auto captured = QSharedPointer<QString>::create();
        process->setProcessChannelMode(QProcess::MergedChannels);
        const QString program = daemonProgram();
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
                this, [this, process, captured](int code, QProcess::ExitStatus exitStatus) {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardOutput());
            *captured += chunk;
            appendOutput(chunk);
            if (exitStatus == QProcess::NormalExit && code == 0) {
                applyCalibrationOutput(*captured);
                statusBar()->showMessage(QStringLiteral("Calibration succeeded"), 4000);
            } else if (exitStatus != QProcess::NormalExit) {
                appendOutputLine(QStringLiteral("calibration crashed"));
                statusBar()->showMessage(QStringLiteral("Calibration crashed"), 4000);
            } else {
                appendOutputLine(QStringLiteral("exit status: %1").arg(code));
                statusBar()->showMessage(QStringLiteral("Calibration failed (exit %1)").arg(code), 4000);
            }
            process->deleteLater();
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
        aimAccelCalibrationEnabled->setChecked(false);
        aimMotionPlusCalibrationEnabled->setChecked(false);
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

    void applyDumpedConfig(const QString &text)
    {
        resetConfigForm();
        const QStringList lines = text.split(QLatin1Char('\n'));
        for (QString line : lines) {
            const int comment = line.indexOf(QLatin1Char('#'));
            if (comment >= 0)
                line.truncate(comment);
            line = line.trimmed();
            if (line.isEmpty())
                continue;
            const int equal = line.indexOf(QLatin1Char('='));
            if (equal <= 0)
                continue;
            applyConfigValue(line.left(equal).trimmed(), line.mid(equal + 1).trimmed());
        }
    }

    void loadConfigFromPath(const QString &path, bool reportErrors)
    {
        if (loadingConfig)
            return;

        const QString selected = path.trimmed();
        if (selected.isEmpty()) {
            if (reportErrors)
                QMessageBox::warning(
                    this,
                    QStringLiteral("No configuration target"),
                    QStringLiteral("Choose a configuration file or restore the default path."));
            return;
        }

        QStringList arguments{QStringLiteral("--dump-config")};
        if (isExplicitConfigPath(selected))
            arguments = QStringList{QStringLiteral("--config"), selected} + arguments;

        loadingConfig = true;
        loadButton->setEnabled(false);
        auto *process = new QProcess(this);
        auto standardOutput = QSharedPointer<QString>::create();
        auto standardError = QSharedPointer<QString>::create();
        const QString program = daemonProgram();
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(program, arguments));
        connect(process, &QProcess::readyReadStandardOutput, this,
                [this, process, standardOutput]() {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardOutput());
            *standardOutput += chunk;
            appendOutput(chunk);
        });
        connect(process, &QProcess::readyReadStandardError, this,
                [this, process, standardError]() {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardError());
            *standardError += chunk;
            appendOutput(chunk);
        });
        connect(process, &QProcess::errorOccurred, this,
                [this, process, reportErrors](QProcess::ProcessError error) {
            if (error != QProcess::FailedToStart)
                return;
            const QString detail = process->errorString();
            appendOutputLine(QStringLiteral("config load failed to start: ") + detail);
            loadingConfig = false;
            loadButton->setEnabled(true);
            statusBar()->showMessage(QStringLiteral("Effective config could not be loaded"), 5000);
            if (reportErrors)
                QMessageBox::warning(this, QStringLiteral("Cannot load effective config"), detail);
            process->deleteLater();
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this,
                [this, process, standardOutput, standardError, selected, reportErrors](
                    int code,
                    QProcess::ExitStatus exitStatus) {
            const QString outputRemainder =
                QString::fromLocal8Bit(process->readAllStandardOutput());
            const QString errorRemainder =
                QString::fromLocal8Bit(process->readAllStandardError());
            *standardOutput += outputRemainder;
            *standardError += errorRemainder;
            appendOutput(outputRemainder);
            appendOutput(errorRemainder);

            loadingConfig = false;
            loadButton->setEnabled(true);
            if (exitStatus == QProcess::NormalExit && code == 0) {
                applyDumpedConfig(*standardOutput);
                statusBar()->showMessage(
                    isExplicitConfigPath(selected)
                        ? QStringLiteral("Loaded effective configuration from %1").arg(selected)
                        : QStringLiteral("Loaded effective layered configuration"),
                    5000);
            } else {
                const QString outcome = exitStatus == QProcess::NormalExit
                    ? QStringLiteral("wiilandd exited with code %1.").arg(code)
                    : QStringLiteral("wiilandd crashed while loading configuration.");
                const QString detail = standardError->trimmed().isEmpty()
                    ? outcome
                    : outcome + QStringLiteral("\n\n") + standardError->trimmed();
                appendOutputLine(QStringLiteral("config load failed: ") + outcome);
                statusBar()->showMessage(QStringLiteral("Effective config load failed"), 5000);
                if (reportErrors)
                    QMessageBox::warning(
                        this,
                        QStringLiteral("Cannot load effective config"),
                        detail);
            }
            process->deleteLater();
        });
        process->start(program, arguments);
        statusBar()->showMessage(QStringLiteral("Loading effective configuration"));
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
            aimAccelCalibrationEnabled->setChecked(true);
            aimAccelZeroX->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-accel-zero-y")) {
            aimAccelCalibrationEnabled->setChecked(true);
            aimAccelZeroY->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-accel-zero-z")) {
            aimAccelCalibrationEnabled->setChecked(true);
            aimAccelZeroZ->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-motion-plus-bias-x")) {
            aimMotionPlusCalibrationEnabled->setChecked(true);
            aimMotionPlusBiasX->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-motion-plus-bias-y")) {
            aimMotionPlusCalibrationEnabled->setChecked(true);
            aimMotionPlusBiasY->setValue(value.toInt());
        } else if (key == QStringLiteral("aim-motion-plus-bias-z")) {
            aimMotionPlusCalibrationEnabled->setChecked(true);
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

    bool validateConfigForm()
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
        return true;
    }

    QByteArray renderedConfig() const
    {
        QString text;
        QTextStream out(&text);
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
        if (aimAccelCalibrationEnabled->isChecked()) {
            out << "aim-accel-zero-x=" << aimAccelZeroX->value() << "\n";
            out << "aim-accel-zero-y=" << aimAccelZeroY->value() << "\n";
            out << "aim-accel-zero-z=" << aimAccelZeroZ->value() << "\n";
        }
        if (aimMotionPlusCalibrationEnabled->isChecked()) {
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
        return text.toUtf8();
    }

    void setSaving(bool active)
    {
        savingConfig = active;
        saveButton->setEnabled(!active);
        saveAndRestartButton->setEnabled(
            !active && !isExplicitConfigPath(configPath->text()));
    }

    void saveConfig(bool restartAfterSave)
    {
        if (savingConfig || !validateConfigForm())
            return;

        const QString target = configPath->text().trimmed();
        if (target.isEmpty()) {
            QMessageBox::warning(
                this,
                QStringLiteral("No configuration target"),
                QStringLiteral("Choose a configuration file before saving."));
            return;
        }

        const QFileInfo info(target);
        QDir dir = info.dir();
        if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
            QMessageBox::warning(this, QStringLiteral("Cannot create directory"), dir.path());
            return;
        }

        auto temporary = QSharedPointer<QTemporaryFile>::create(
            dir.filePath(QStringLiteral(".wiiland-config-XXXXXX")));
        if (!temporary->open()) {
            QMessageBox::warning(
                this,
                QStringLiteral("Cannot prepare config validation"),
                temporary->errorString());
            return;
        }
        auto rendered = QSharedPointer<QByteArray>::create(renderedConfig());
        if (temporary->write(*rendered) != rendered->size() || !temporary->flush()) {
            const QString detail = temporary->errorString();
            temporary->close();
            QMessageBox::warning(
                this,
                QStringLiteral("Cannot prepare config validation"),
                detail);
            return;
        }
        temporary->close();

        const QStringList arguments{
            QStringLiteral("--check-config"),
            QStringLiteral("--config"),
            temporary->fileName(),
        };
        const QString program = daemonProgram();
        appendOutputLine(QStringLiteral("$ ") + quoteCommand(program, arguments));
        setSaving(true);

        auto *process = new QProcess(this);
        auto standardOutput = QSharedPointer<QString>::create();
        auto standardError = QSharedPointer<QString>::create();
        connect(process, &QProcess::readyReadStandardOutput, this,
                [this, process, standardOutput]() {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardOutput());
            *standardOutput += chunk;
            appendOutput(chunk);
        });
        connect(process, &QProcess::readyReadStandardError, this,
                [this, process, standardError]() {
            const QString chunk = QString::fromLocal8Bit(process->readAllStandardError());
            *standardError += chunk;
            appendOutput(chunk);
        });
        connect(process, &QProcess::errorOccurred, this,
                [this, process, temporary](QProcess::ProcessError error) {
            if (error != QProcess::FailedToStart)
                return;
            const QString detail = process->errorString();
            appendOutputLine(QStringLiteral("config validation failed to start: ") + detail);
            setSaving(false);
            statusBar()->showMessage(QStringLiteral("Configuration was not saved"), 5000);
            QMessageBox::warning(
                this,
                QStringLiteral("Cannot validate configuration"),
                detail + QStringLiteral("\n\nThe existing configuration was not changed."));
            process->deleteLater();
        });
        connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
                this,
                [this,
                 process,
                 temporary,
                 rendered,
                 standardOutput,
                 standardError,
                 target,
                 restartAfterSave](int code, QProcess::ExitStatus exitStatus) {
            const QString outputRemainder =
                QString::fromLocal8Bit(process->readAllStandardOutput());
            const QString errorRemainder =
                QString::fromLocal8Bit(process->readAllStandardError());
            *standardOutput += outputRemainder;
            *standardError += errorRemainder;
            appendOutput(outputRemainder);
            appendOutput(errorRemainder);

            if (exitStatus != QProcess::NormalExit || code != 0) {
                const QString outcome = exitStatus == QProcess::NormalExit
                    ? QStringLiteral("wiilandd rejected the rendered configuration (exit %1).").arg(code)
                    : QStringLiteral("wiilandd crashed while validating the rendered configuration.");
                QString daemonDetails = standardError->trimmed();
                if (daemonDetails.isEmpty())
                    daemonDetails = standardOutput->trimmed();
                const QString detail = daemonDetails.isEmpty()
                    ? outcome
                    : outcome + QStringLiteral("\n\n") + daemonDetails;
                appendOutputLine(QStringLiteral("config validation failed: ") + outcome);
                setSaving(false);
                statusBar()->showMessage(QStringLiteral("Configuration was not saved"), 5000);
                QMessageBox::warning(
                    this,
                    QStringLiteral("Invalid configuration"),
                    detail + QStringLiteral("\n\nThe existing configuration was not changed."));
                process->deleteLater();
                return;
            }

            QSaveFile file(target);
            file.setDirectWriteFallback(false);
            if (!file.open(QIODevice::WriteOnly | QIODevice::Text) ||
                file.write(*rendered) != rendered->size()) {
                const QString detail = file.errorString();
                file.cancelWriting();
                setSaving(false);
                QMessageBox::warning(
                    this,
                    QStringLiteral("Cannot write configuration"),
                    detail + QStringLiteral("\n\nThe existing configuration was not changed."));
                process->deleteLater();
                return;
            }
            if (!file.commit()) {
                const QString detail = file.errorString();
                setSaving(false);
                QMessageBox::warning(
                    this,
                    QStringLiteral("Cannot replace configuration"),
                    detail + QStringLiteral("\n\nThe existing configuration was not changed."));
                process->deleteLater();
                return;
            }

            setSaving(false);
            const bool serviceManagedTarget = !isExplicitConfigPath(target);
            statusBar()->showMessage(
                serviceManagedTarget
                    ? QStringLiteral("Saved %1 — daemon restart required").arg(target)
                    : QStringLiteral("Saved %1 — start wiilandd with --config to apply it").arg(target),
                7000);
            process->deleteLater();
            if (restartAfterSave && serviceManagedTarget) {
                runServiceAction(QStringLiteral("restart"));
                return;
            }

            if (!serviceManagedTarget) {
                QMessageBox::information(
                    this,
                    QStringLiteral("Configuration saved"),
                    QStringLiteral("The configuration was validated and saved to %1.\n\n"
                                   "The installed user service does not load explicit configuration "
                                   "paths. Run %2 to apply this file.")
                        .arg(target,
                             quoteCommand(
                                 daemonProgram(),
                                 {QStringLiteral("--config"), target})));
                return;
            }

            QMessageBox message(
                QMessageBox::Information,
                QStringLiteral("Configuration saved"),
                QStringLiteral("The configuration was validated and saved to %1.\n\n"
                               "Restart wiilandd.service to apply it.")
                    .arg(target),
                QMessageBox::NoButton,
                this);
            auto *restart = message.addButton(
                QStringLiteral("Restart daemon"),
                QMessageBox::AcceptRole);
            message.addButton(QStringLiteral("Later"), QMessageBox::RejectRole);
            message.exec();
            if (message.clickedButton() == restart)
                runServiceAction(QStringLiteral("restart"));
        });
        process->start(program, arguments);
        statusBar()->showMessage(QStringLiteral("Validating configuration before save"));
    }

    QLineEdit *wiilanddPath = nullptr;
    QLineEdit *configPath = nullptr;
    QLineEdit *deviceSelector = nullptr;
    QComboBox *traceFilter = nullptr;
    QComboBox *traceProfile = nullptr;
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
    QCheckBox *aimAccelCalibrationEnabled = nullptr;
    QCheckBox *aimMotionPlusCalibrationEnabled = nullptr;
    QSpinBox *aimCalibrationDuration = nullptr;
    QSpinBox *aimAccelZeroX = nullptr;
    QSpinBox *aimAccelZeroY = nullptr;
    QSpinBox *aimAccelZeroZ = nullptr;
    QSpinBox *aimMotionPlusBiasX = nullptr;
    QSpinBox *aimMotionPlusBiasY = nullptr;
    QSpinBox *aimMotionPlusBiasZ = nullptr;
    QCheckBox *aimInvertY = nullptr;
    QLabel *configScope = nullptr;
    QLabel *serviceStatus = nullptr;
    QPushButton *loadButton = nullptr;
    QPushButton *saveButton = nullptr;
    QPushButton *saveAndRestartButton = nullptr;
    QPushButton *serviceRefresh = nullptr;
    QPushButton *serviceStart = nullptr;
    QPushButton *serviceStop = nullptr;
    QPushButton *serviceRestart = nullptr;
    QHash<QString, QComboBox *> desktopActions;
    QTableWidget *rules = nullptr;
    QPlainTextEdit *output = nullptr;
    QProcess *traceProcess = nullptr;
    QProcess *serviceProcess = nullptr;
    bool loadingConfig = false;
    bool savingConfig = false;
};

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    QApplication::setApplicationName(QStringLiteral("wiiland-config"));
    QApplication::setApplicationDisplayName(QStringLiteral("WiiLand Control Center"));
    QApplication::setOrganizationName(QStringLiteral("WiiLand"));
    QGuiApplication::setDesktopFileName(
        QStringLiteral("io.github.philosophimoonbeam.wiiland-config"));
    MainWindow window;
    window.show();
    if (qEnvironmentVariable("WIILAND_CONFIG_SMOKE_TEST") == QStringLiteral("1")) {
        QTextStream stream(stdout);
        if (!window.writeSmokeReport(stream))
            return EXIT_FAILURE;
        QTimer::singleShot(50, &app, &QApplication::quit);
    }
    return app.exec();
}
