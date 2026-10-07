import QtQuick
import Quickshell
import Quickshell.Io
import qs.Ui

BarWidget {
    id: root

    property bool isHot: false
    moduleName: "io.github.ianmove.thermalwatch"
    property string tempText: "--°"

    implicitHeight: button.implicitHeight
    implicitWidth: button.implicitWidth

    Timer {
        interval: 2000
        repeat: true
        running: true
        triggeredOnStart: true

        onTriggered: tempCheck.running = true
    }

    Process {
        id: tempCheck

        command: ["sh", "-c", "for p in /sys/class/hwmon/hwmon*; do if [ -f \"$p/name\" ] && grep -qE 'k10temp|coretemp|acpitz' \"$p/name\" 2>/dev/null; then cat \"$p/temp1_input\" 2>/dev/null && break; fi; done"]

        stdout: StdioCollector {
            onStreamFinished: {
                let v = parseInt(text.trim());
                if (!isNaN(v) && v > 0) {
                    let c = Math.round(v / 1000);
                    root.tempText = c + "°";
                    root.isHot = (c >= 85);
                }
            }
        }
    }

    WidgetButton {
        id: button

        anchors.fill: parent
        bar: root.bar
        text: (root.isHot ? "🔥 " : "󰔏 ") + root.tempText
        tooltipText: "ThermalWatch · Clic: abrir monitor · Clic derecho: modo pánico"

        onPressed: function(btn) {
            if (!root.bar)
                return;
            if (btn === Qt.RightButton) {
                root.bar.run("thermalwatch --panic");
            } else {
                root.bar.run("thermalwatch");
            }
        }
    }
}
