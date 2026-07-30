import QtQuick
import Quickshell.Io
import qs.Ui

BarWidget {
  id: root
  moduleName: "local.omastat"

  property string displayText: "Oma"
  property string tooltip: "Omastat"
  property bool refreshRunning: false
  property var rows: []

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()

  Timer {
    interval: Math.max(15, Number(root.setting("refreshIntervalSec", 60))) * 1000
    running: true
    repeat: true
    triggeredOnStart: true
    onTriggered: root.refresh()
  }

  Process {
    id: reportProcess
    running: false
    onRunningChanged: root.refreshRunning = running
    onExited: function(exitCode) {
      if (exitCode !== 0) {
        root.displayText = "Oma"
        root.tooltip = "Omastat report failed"
      }
    }

    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.parseReport(text)
    }

    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: if (text.trim() !== "") console.warn("omastat", text.trim())
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.displayText
    fontSize: 12
    horizontalMargin: 8
    tooltipText: root.tooltip
    active: root.refreshRunning
    onPressed: function(button) {
      if (!root.bar) return
      if (button === Qt.RightButton) root.refresh()
      else root.bar.run("xdg-terminal-exec omastat today")
    }
  }

  function refresh() {
    if (reportProcess.running) return
    reportProcess.command = splitCommand(String(root.setting("command", "omastat --json today")))
    reportProcess.running = true
  }

  function parseReport(text) {
    try {
      rows = JSON.parse(String(text || "[]"))
    } catch (error) {
      rows = []
      displayText = "Oma"
      tooltip = "Omastat report parse failed"
      return
    }

    var totalFocused = 0
    for (var i = 0; i < rows.length; i++) totalFocused += Number(rows[i].focused_seconds || 0)

    if (rows.length === 0 || totalFocused <= 0) {
      displayText = "Oma 0s"
      tooltip = "No focused time today"
      return
    }

    var top = rows[0]
    displayText = shortApp(String(top.app_class || "App")) + " " + formatDuration(Number(top.focused_seconds || 0))
    tooltip = "Today: " + formatDuration(totalFocused) + " focused"
      + "\\nTop: " + String(top.app_class || "App")
      + " (" + formatDuration(Number(top.focused_seconds || 0)) + ")"
  }

  function splitCommand(command) {
    return String(command || "").trim().split(/\\s+/).filter(function(part) { return part.length > 0 })
  }

  function shortApp(app) {
    var value = app.replace(/^com\\./, "").replace(/^org\\./, "")
    var parts = value.split(".")
    value = parts[parts.length - 1] || value
    return value.length > 10 ? value.substring(0, 9) + "." : value
  }

  function formatDuration(seconds) {
    seconds = Math.max(0, Math.floor(seconds))
    if (seconds < 60) return seconds + "s"
    var minutes = Math.floor(seconds / 60)
    var hours = Math.floor(minutes / 60)
    if (hours > 0) return hours + "h " + String(minutes % 60).padStart(2, "0") + "m"
    return minutes + "m"
  }
}
