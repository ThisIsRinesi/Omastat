import QtQuick
import QtQuick.Layouts
import Quickshell.Io
import qs.Commons
import "Model.js" as Model

ColumnLayout {
  id: root
  property bool active: false
  property color foreground: "white"
  property color dim: "gray"
  property string fontFamily: Style.font.family
  property var report: null
  property string errorText: ""
  property bool checking: false
  property string output: ""
  property bool outputReady: false
  property bool exitReady: false
  property int exitCode: 0
  readonly property var status: Model.trackingStatusView(report)
  spacing: Style.space(4)

  onActiveChanged: if (active) refresh()
  Component.onCompleted: if (active) refresh()

  function refresh() {
    if (checking) return
    checking = true
    errorText = ""
    output = ""
    outputReady = false
    exitReady = false
    statusProcess.running = true
  }

  function finish() {
    if (!outputReady || !exitReady) return
    checking = false
    report = null
    if (exitCode !== 0) {
      errorText = "Status unavailable. Run omastat doctor to check the tracker and database."
      return
    }
    try {
      var parsed = JSON.parse(output)
      if (!parsed || typeof parsed.tracker !== "string" || typeof parsed.browser !== "string"
          || typeof parsed.checked_at !== "number") throw new Error("Invalid status")
      report = parsed
    } catch (error) {
      errorText = "Status unavailable. Update the backend and widget together."
    }
  }

  Timer { interval: 30000; running: root.active; repeat: true; onTriggered: root.refresh() }
  Process {
    id: statusProcess
    command: ["bash", "-c", "PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; exec omastat tracking-status"]
    onExited: function(code) { root.exitCode = code; root.exitReady = true; root.finish() }
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: { root.output = text; root.outputReady = true; root.finish() }
    }
    stderr: StdioCollector { waitForEnd: true }
  }

  StatusLabel { text: "Tracking status"; font.bold: true }
  StatusLabel {
    visible: root.checking || root.errorText !== ""
    text: root.checking ? "Checking…" : root.errorText
    color: root.dim
  }
  StatusLabel { visible: root.report !== null; text: "Tracker · " + root.status.tracker }
  StatusLabel { visible: root.report !== null; text: root.status.trackerDetail; color: root.dim }
  StatusLabel { visible: root.report !== null; text: "Browser · " + root.status.browser }
  StatusLabel { visible: root.report !== null; text: root.status.browserDetail; color: root.dim }
  StatusLabel {
    visible: root.report !== null
    text: root.report ? "Checked " + new Date(root.report.checked_at * 1000).toLocaleTimeString() : ""
    color: root.dim
    font.pixelSize: Style.font.caption
  }

  component StatusLabel: Text {
    Layout.fillWidth: true
    color: root.foreground
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
    wrapMode: Text.Wrap
    textFormat: Text.PlainText
    Accessible.role: Accessible.StaticText
    Accessible.name: text
  }
}
