import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "local.omastat"

  property string displayText: "󰔟"
  property string tooltip: "Omastat"
  property string statusText: "Not loaded"
  property string errorText: ""
  property string updatedText: ""
  property bool refreshRunning: false
  property bool refreshQueued: false
  property var rows: []
  property var reportApps: []
  property var reportInsights: []
  property var widgetInsight: null
  property var daily: []
  property string todayKey: ""
  property string periodLabel: "Today"
  property int totalFocused: 0
  property int totalOpen: 0

  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property real openPanelIndicatorWidth: button.labelWidth
  readonly property string glyph: "󰔟"
  readonly property string defaultOverviewCommand: "bash -lc 'set -e; PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; out=\"${XDG_CACHE_HOME:-$HOME/.cache}/omastat/overview.html\"; mkdir -p \"${out%/*}\"; omastat export --lens day --output \"$out\"; xdg-open \"$out\" >/dev/null 2>&1 &'"
  readonly property bool iconOnly: {
    var value = root.setting("iconOnly", false)
    return value === true || value === "true"
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()
  onRowsChanged: injectPanel()
  onReportAppsChanged: injectPanel()
  onReportInsightsChanged: injectPanel()
  onWidgetInsightChanged: injectPanel()
  onDailyChanged: injectPanel()
  onTodayKeyChanged: injectPanel()
  onPeriodLabelChanged: injectPanel()
  onTotalFocusedChanged: injectPanel()
  onTotalOpenChanged: injectPanel()
  onStatusTextChanged: injectPanel()
  onErrorTextChanged: injectPanel()
  onUpdatedTextChanged: injectPanel()

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
        root.errorText = "Report command failed"
        root.statusText = "Command failed"
        root.displayText = root.glyph + " !"
        root.tooltip = "Omastat report failed"
      }
      if (root.refreshQueued) {
        root.refreshQueued = false
        root.refresh()
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

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  IpcHandler {
    target: "local.omastat"

    function refresh() { root.refresh() }
    function open() { root.open() }
    function close() { root.close() }
    function show() { root.open() }
    function hide() { root.close() }
    function toggle() { root.togglePanel() }
    function overview() { root.openOverviewReport() }
    function status() {
      console.log("local.omastat status: opened=" + root.opened
        + " total=" + root.formatDuration(root.totalFocused)
        + " apps=" + root.rows.length
        + " updated=" + root.updatedText)
    }
  }

  WidgetButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.vertical || root.iconOnly ? root.glyph : root.displayText
    fontSize: 12
    horizontalMargin: 8
    tooltipText: root.tooltip
    active: root.refreshRunning || root.errorText !== ""

    onPressed: function(button) {
      if (button === Qt.RightButton) root.toggleIconOnly()
      else if (button === Qt.MiddleButton) root.openOverviewReport()
      else root.togglePanel()
    }
  }

  function refresh() {
    if (reportProcess.running) {
      refreshQueued = true
      return
    }
    errorText = ""
    statusText = "Refreshing"
    reportProcess.command = shellCommand(String(root.setting("command", "omastat summary")))
    reportProcess.running = true
  }

  function parseReport(text) {
    var parsed = []
    try {
      parsed = JSON.parse(String(text || "[]"))
    } catch (error) {
      rows = []
      reportApps = []
      reportInsights = []
      widgetInsight = null
      daily = []
      todayKey = ""
      periodLabel = "Today"
      totalFocused = 0
      totalOpen = 0
      displayText = root.glyph + " !"
      tooltip = "Omastat report parse failed"
      errorText = "Report JSON parse failed"
      statusText = "Parse failed"
      return
    }

    if (Array.isArray(parsed)) {
      rows = parsed
      reportApps = []
      reportInsights = []
      widgetInsight = null
      daily = []
      todayKey = ""
      periodLabel = "Today"
      totalFocused = sumSeconds(rows, "focused_seconds")
      totalOpen = sumSeconds(rows, "open_seconds")
    } else if (parsed && typeof parsed === "object") {
      rows = Array.isArray(parsed.rows) ? parsed.rows : []
      reportApps = Array.isArray(parsed.apps) ? normalizeApps(parsed.apps) : []
      reportInsights = Array.isArray(parsed.insights) ? normalizeInsights(parsed.insights) : []
      widgetInsight = parsed.widget_insight && typeof parsed.widget_insight === "object" ? normalizeWidgetInsight(parsed.widget_insight) : null
      daily = Array.isArray(parsed.daily) ? normalizeDaily(parsed.daily) : []
      todayKey = String(parsed.today_key || "")
      periodLabel = parsed.period && typeof parsed.period === "object" ? String(parsed.period.label || "Today") : "Today"
      totalFocused = numericField(parsed, "total_focused_seconds", sumSeconds(rows, "focused_seconds"))
      totalOpen = numericField(parsed, "total_open_seconds", sumSeconds(rows, "open_seconds"))
    } else {
      rows = []
      reportApps = []
      reportInsights = []
      widgetInsight = null
      daily = []
      todayKey = ""
      periodLabel = "Today"
      totalFocused = 0
      totalOpen = 0
    }
    updatedText = Qt.formatTime(new Date(), "HH:mm:ss")

    if ((rows.length === 0 && reportApps.length === 0) || totalFocused <= 0) {
      displayText = root.glyph + " 0s"
      tooltip = "No focused time today"
      statusText = "No focused time today"
      errorText = ""
      return
    }

    var top = reportApps.length > 0 ? reportApps[0] : rows[0]
    displayText = root.glyph + " " + formatDuration(totalFocused)
    tooltip = "Today: " + formatDuration(totalFocused) + " focused"
      + "\nOpen: " + formatDuration(totalOpen)
      + "\nTop: " + topAppLabel(top)
      + " (" + formatDuration(topAppSeconds(top)) + ")"
    if (widgetInsight && widgetInsight.text)
      tooltip += "\nInsight: " + String(widgetInsight.text)
    statusText = widgetInsight && widgetInsight.text
      ? String(widgetInsight.text)
      : Math.max(rows.length, reportApps.length) + " apps with focus"
    errorText = ""
  }

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
    if ("rows" in target) target.rows = root.rows
    if ("reportApps" in target) target.reportApps = root.reportApps
    if ("reportInsights" in target) target.reportInsights = root.reportInsights
    if ("widgetInsight" in target) target.widgetInsight = root.widgetInsight
    if ("daily" in target) target.daily = root.daily
    if ("todayKey" in target) target.todayKey = root.todayKey
    if ("periodLabel" in target) target.periodLabel = root.periodLabel
    if ("totalFocused" in target) target.totalFocused = root.totalFocused
    if ("totalOpen" in target) target.totalOpen = root.totalOpen
    if ("statusText" in target) target.statusText = root.statusText
    if ("errorText" in target) target.errorText = root.errorText
    if ("updatedText" in target) target.updatedText = root.updatedText
  }

  function open() {
    if (panelLoader.item) panelLoader.item.open()
  }

  function close() {
    if (panelLoader.item) panelLoader.item.close()
  }

  function togglePanel() {
    if (panelLoader.item) panelLoader.item.toggle()
  }

  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  function openOverviewReport() {
    if (!root.bar) return
    var command = String(root.setting("overviewCommand", root.defaultOverviewCommand)).trim()
    root.bar.run(command.length > 0 ? command : root.defaultOverviewCommand)
  }

  function openTerminalReport() {
    openOverviewReport()
  }

  function toggleIconOnly() {
    var next = !root.iconOnly
    var entry = { id: root.moduleName }
    var current = root.settings || {}
    for (var key in current) if (key !== "id") entry[key] = current[key]
    entry.iconOnly = next
    root.settings = entry
    if (root.bar && root.bar.shell && typeof root.bar.shell.updateEntryInline === "function")
      root.bar.shell.updateEntryInline(root.moduleName, entry)
  }

  function shellCommand(command) {
    var value = String(command || "").trim()
    if (value.length === 0) value = "omastat summary"
    return ["bash", "-lc", "PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:$PATH\"; " + value]
  }

  function sumSeconds(list, key) {
    var total = 0
    for (var i = 0; i < list.length; i++) total += Number(list[i][key] || 0)
    return Math.max(0, Math.floor(total))
  }

  function numericField(object, key, fallback) {
    var value = object ? object[key] : undefined
    var number = Number(value)
    if (value === undefined || value === null || isNaN(number)) return Math.max(0, Math.floor(Number(fallback) || 0))
    return Math.max(0, Math.floor(number))
  }

  function normalizeApps(list) {
    var output = []
    for (var i = 0; i < list.length; i++) {
      var item = list[i] || {}
      var seconds = numericField(item, "focused_seconds", numericField(item, "seconds", 0))
      if (seconds <= 0) continue
      var rawShare = item.share !== undefined && item.share !== null ? Number(item.share) : Number(item.pct || 0) / 100
      if (isNaN(rawShare)) rawShare = 0
      output.push({
        app: String(item.label || item.app || item.app_class || "App"),
        seconds: seconds,
        open_seconds: numericField(item, "open_seconds", 0),
        pct: Math.round(Math.max(0, Math.min(1, rawShare)) * 100)
      })
    }
    return output
  }

  function normalizeDaily(list) {
    var output = []
    for (var i = 0; i < list.length; i++) {
      var item = list[i] || {}
      var focused = numericField(
        item,
        "focused_seconds",
        numericField(
          item,
          "focus_seconds",
          numericField(item, "seconds", numericField(item, "total_focused_seconds", 0))
        )
      )
      var open = numericField(item, "open_seconds", numericField(item, "total_open_seconds", 0))
      var idle = numericField(item, "idle_seconds", 0)
      var locked = numericField(item, "locked_seconds", 0)
      var sleep = numericField(item, "sleep_seconds", 0)
      var unobserved = numericField(item, "unobserved_seconds", 0)
      output.push({
        date: String(item.date || item.key || ""),
        label: String(item.label || item.day || item.date || ""),
        focused_seconds: focused,
        open_seconds: open,
        idle_seconds: idle,
        locked_seconds: locked,
        sleep_seconds: sleep,
        unobserved_seconds: unobserved,
        excluded_seconds: numericField(item, "excluded_seconds", idle + locked + sleep + unobserved)
      })
    }
    return output
  }

  function normalizeInsights(list) {
    var output = []
    for (var i = 0; i < list.length; i++) {
      var item = list[i] || {}
      var label = String(item.title || item.label || item.kind || "")
      var value = String(item.value || "")
      if (label.length === 0 && value.length === 0) continue
      output.push({
        label: friendlyInsightLabel(item, label),
        value: value,
        detail: friendlyInsightDetail(item),
        category: String(item.category || ""),
        confidence: String(item.confidence || ""),
        tone: String(item.tone || "")
      })
    }
    return output
  }

  function normalizeWidgetInsight(item) {
    var label = friendlyInsightLabel(item, String(item.title || item.label || "Insight"))
    var value = String(item.value || "")
    var text = value.length > 0 ? label + ": " + value : label
    return {
      title: label,
      value: value,
      tone: String(item.tone || ""),
      text: text
    }
  }

  function friendlyInsightLabel(item, fallback) {
    var kind = String(item && item.kind || "")
    switch (kind) {
      case "top-app": return "Top app"
      case "day-comparison": return "Compared with yesterday"
      case "period-comparison": return "Compared with last period"
      case "best-day": return "Best day"
      case "worst-active-day": return "Lightest day"
      case "current-streak": return "Current streak"
      case "longest-streak": return "Best streak"
      case "peak-focus-hour": return "Top hour"
      case "peak-focus-weekday": return "Top weekday"
      case "deep-work-blocks": return "Long sessions"
      case "app-switch-rate": return "App changes"
      case "fragmented-app": return "Most interrupted app"
      case "focus-density": return "Focus share"
      case "effective-apps": return "Focus spread"
      case "strongest-workspace": return "Top workspace"
      case "workspace-app-affinity": return "Workspace pairing"
      case "idle-excluded": return "Away time"
      case "locked-excluded": return "Locked time"
      case "sleep-excluded": return "Sleep time"
      case "unobserved-excluded": return "Tracker off time"
      case "excluded-impact": return "Not counted time"
      case "focus-anomaly": return "Unusual focus time"
      case "app-anomaly": return "Unusual app time"
      case "hour-anomaly": return "Unusual hour"
      case "unobserved-anomaly": return "Tracker off gap"
    }
    var label = String(fallback || "")
    if (label === "Top app share") return "Top app"
    if (label === "Deep work blocks") return "Long sessions"
    if (label === "Focus density") return "Focus share"
    if (label === "Effective app count") return "Focus spread"
    if (label === "Unobserved time excluded") return "Tracker off time"
    if (label === "Excluded time impact") return "Not counted time"
    if (label === "vs yesterday") return "Compared with yesterday"
    if (label.indexOf("Densest app") >= 0) return "Highest focus share app"
    if (label.indexOf("Lowest-density app") >= 0) return "Lowest focus share app"
    return label
  }

  function friendlyInsightDetail(item) {
    var kind = String(item && item.kind || "")
    switch (kind) {
      case "deep-work-blocks":
        return "Counts long focus sessions at or above your long session threshold."
      case "app-switch-rate":
        return "Counts how often focus moved from one app to another."
      case "focus-density":
        return "Shows how much open app time was focused."
      case "unobserved-excluded":
        return "Tracker off time was not counted as focus."
      case "excluded-impact":
        return "Shows how much time was left out because it was away, locked, sleep, or tracker off time."
    }
    var detail = String(item && (item.explanation || item.detail) || "")
    return detail
      .replace(/focus density/gi, "focus share")
      .replace(/density/gi, "focus share")
      .replace(/unobserved/gi, "tracker off")
      .replace(/excluded/gi, "not counted")
      .replace(/deep-work/gi, "long session")
  }

  function topAppLabel(app) {
    if (!app) return "App"
    return String(app.app || app.label || app.app_class || "App")
  }

  function topAppSeconds(app) {
    if (!app) return 0
    return numericField(app, "seconds", numericField(app, "focused_seconds", 0))
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
