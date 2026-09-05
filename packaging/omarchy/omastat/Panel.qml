import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Controls as Controls
import Quickshell.Io
import qs.Commons
import qs.Ui as Ui
import "Model.js" as Model

Ui.Panel {
  id: root
  moduleName: "local.omastat"
  manageIpc: false
  property var anchorItem: null
  property var hostWidget: null
  property string selectedLens: "day"
  property int selectedOffset: 0
  property bool refreshRunning: false
  property bool panelDataLoaded: false
  property var rows: []
  property var reportApps: []
  property var browserActivity: []
  property var summaryTopApp: null
  property var reportInsights: []
  property var widgetInsight: null
  property var daily: []
  property var heatmap: []
  property string todayKey: ""
  property string lensLabel: "DAY"
  property string periodLabel: "Today"
  property real totalFocused: 0
  property real totalOpen: 0
  property real totalElapsed: 0
  property real totalObserved: 0
  property real totalIdle: 0
  property real totalLocked: 0
  property real totalSleep: 0
  property real totalUnobserved: 0
  property string statusText: ""
  property string errorText: ""
  property string updatedText: ""
  property var activityAnalytics: ({})
  property var activityDetail: null
  property string selectedActivityKind: ""
  property string selectedActivityKey: ""
  property bool detailRunning: false
  property string detailError: ""
  property string activityType: "app"
  property bool showAllActivities: false
  property bool showAllInsights: false
  property string expandedInsight: ""
  property string chartReadout: ""
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property color accent: Color.accent
  readonly property color dim: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.75)
  readonly property color line: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.18)
  readonly property color fill: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.06)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property bool wide: panel.contentWidth >= Style.space(900)
  readonly property bool selected: selectedActivityKey.length > 0
  readonly property var detail: selected ? (activityDetail || {}) : activityAnalytics
  readonly property var stats: selected && detail.activities && detail.activities.length ? detail.activities[0] : null
  readonly property string activityLabel: stats ? String(stats.label) : (selected ? selectedActivityKey : "All activity")
  readonly property real shownSeconds: selected ? (stats ? Number(stats.focused_seconds) : 0) : totalFocused
  readonly property var shownDaily: selected ? (detail.daily || []) : daily
  readonly property var shownHeat: selected ? (detail.heatmap || []) : heatmap
  readonly property var hours: Model.hourlyCells(shownHeat)
  readonly property var trend: Model.activityCells(shownDaily, selectedLens)
  readonly property var heatCells: Model.heatmapCells(shownHeat)
  readonly property var insights: Model.diverseInsights(selected ? (detail.insights || []) : reportInsights)
  readonly property var filteredActivities: {
    var list = activityAnalytics.activities || []
    var query = search.text.toLowerCase().trim()
    return list.filter(function(item) {
      return item.kind === root.activityType && (!query || String(item.label).toLowerCase().indexOf(query) >= 0 || String(item.key).toLowerCase().indexOf(query) >= 0)
    })
  }
  readonly property string baselineText: detail.baseline_start
    ? Model.insightDateRange(detail.baseline_start, detail.baseline_end) : "Up to eight weeks of history"
  onSelectedLensChanged: resetView()
  onSelectedOffsetChanged: resetView()
  onSelectedActivityKeyChanged: { chartReadout = ""; showAllInsights = false; scroll.contentY = 0 }

  readonly property color faint: dim
  readonly property color noFill: "transparent"
  readonly property color hairline: line
  readonly property color track: fill
  readonly property var sliceColors: Model.sliceColors(12, String(accent))
  readonly property var composition: {
    var items = (activityAnalytics.activities || []).filter(function(a) { return a.kind === root.activityType && Number(a.focused_seconds) > 0 })
    var total = items.reduce(function(n, a) { return n + Number(a.focused_seconds) }, 0)
    return items.map(function(a) { return { app: a.label, app_class: a.key, kind: a.kind, seconds: Number(a.focused_seconds), pct: total > 0 ? Math.round(Number(a.focused_seconds) / total * 100) : 0 } })
  }
  readonly property var compositionColors: Model.stableAppColors(composition, String(accent))
  readonly property var lineDays: selectedLens === "month" ? Model.trendDays(shownDaily, "", "month") : trend
  readonly property var calendarCells: Model.monthCells(shownDaily, "month")
  readonly property var calendarWeeks: Model.monthWeekCells(shownDaily)
  readonly property var calendarWeekdays: Model.weekdayFocusCells(shownHeat)
  function formatDuration(seconds) { return Model.fmt(seconds) }
  function sliceColor(index, alpha) {
    return colorFromHex(String(root.sliceColors[index] || Color.accent), alpha)
  }

  function colorFromHex(hex, alpha) {
    var clean = String(hex || "").replace(/[#\s]/g, "")
    var r = parseInt(clean.substr(0, 2), 16) / 255
    var g = parseInt(clean.substr(2, 2), 16) / 255
    var b = parseInt(clean.substr(4, 2), 16) / 255
    if (isNaN(r) || isNaN(g) || isNaN(b)) return Qt.rgba(root.accent.r, root.accent.g, root.accent.b, alpha)
    return Qt.rgba(r, g, b, alpha)
  }

  function clamp01(value) {
    return Math.max(0, Math.min(1, Number(value || 0)))
  }

  function withAlpha(colorValue, alpha) {
    return Qt.rgba(colorValue.r, colorValue.g, colorValue.b, root.clamp01(alpha))
  }

  function canvasColor(colorValue, alpha) {
    var a = alpha === undefined ? colorValue.a : alpha
    return "rgba("
      + Math.round(colorValue.r * 255) + ","
      + Math.round(colorValue.g * 255) + ","
      + Math.round(colorValue.b * 255) + ","
      + root.clamp01(a) + ")"
  }


  function resetView() { chartReadout = ""; expandedInsight = ""; showAllInsights = false; scroll.contentY = 0 }
  function refresh() { if (hostWidget) { hostWidget.refresh(true); hostWidget.refreshDetail() } }
  function setLens(lens) { if (hostWidget) hostWidget.setPeriod(lens, 0) }
  function setLensOffset(lens, offset) { if (hostWidget) hostWidget.setPeriod(lens, offset) }
  function selectActivity(kind, key) { if (kind) activityType = kind; if (hostWidget) hostWidget.setActivity(kind, key) }
  function inspectInsight(item) {
    var key = String(item.title || item.label) + String(item.value)
    expandedInsight = expandedInsight === key ? "" : key
    var support = item.supporting || {}
    var activityKey = String(support.activity_key || support.app_class || "")
    if (activityKey && activityKey !== selectedActivityKey) selectActivity(String(support.activity_kind || "app"), activityKey)
  }
  function maximum(list) {
    var max = 1
    for (var i = 0; i < list.length; i++) max = Math.max(max, Number(list[i].seconds || list[i].focused_seconds || 0))
    return max
  }
  function evidenceText(item) { return Model.insightEvidence(item) }

  IpcHandler {
    target: root.moduleName
    function open() { root.open(); root.refresh() }
    function close() { root.close() }
    function show() { root.open(); root.refresh() }
    function hide() { root.close() }
    function toggle() { root.toggle(); if (root.opened) root.refresh() }
    function refresh() { root.refresh() }
    function status(): string { return root.statusText || "idle" }
    function period(lens: string, offset: string): void { root.setLensOffset(lens, offset) }
    function day() { root.setLens("day") }
    function week() { root.setLens("week") }
    function month() { root.setLens("month") }
    function year() { root.setLens("year") }
    function life() { root.setLens("life") }
    function activity(kind: string, key: string): void { root.selectActivity(kind, key) }
  }

  Ui.KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.hostWidget || root
    bar: root.bar
    open: root.opened
    centerOnBar: true
    margin: Math.max(Style.gapsOut, Style.space(12))
    gap: Math.max(Style.gapsOut, Style.space(8))
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(Math.max(380, Math.min(1600, Number(root.setting("panelWidth", 1160)) || 1160))))
    contentHeight: panel.fittedContentHeight(Style.space(920), Style.space(920))

    Item {
      id: keyCatcher
      anchors.fill: parent
      focus: true
      Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
          if (root.selected) root.selectActivity("", "")
          else root.close()
          event.accepted = true
        } else if (event.key === Qt.Key_Slash) { search.forceActiveFocus(); event.accepted = true }
        else if (event.key === Qt.Key_R) { root.refresh(); event.accepted = true }
      }
      ColumnLayout {
        anchors.fill: parent
        spacing: Style.space(14)
        RowLayout {
          Layout.fillWidth: true
          ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.space(3)
            Label { text: root.periodLabel + (root.selectedOffset === 0 && root.selectedLens !== "day" && root.selectedLens !== "life" ? " to date" : ""); font.pixelSize: Style.font.title; font.bold: true }
            Label { text: "Your computer use, in perspective"; color: root.dim }
          }
          Action { text: "‹"; Accessible.name: "Previous period"; enabled: root.selectedLens !== "life"; onClicked: root.setLensOffset(root.selectedLens, root.selectedOffset - 1) }
          Action { text: "›"; Accessible.name: "Next period"; enabled: root.selectedLens !== "life" && root.selectedOffset < 0; onClicked: root.setLensOffset(root.selectedLens, root.selectedOffset + 1) }
          Action { text: root.refreshRunning ? "Refreshing…" : "Refresh"; enabled: !root.refreshRunning; onClicked: root.refresh() }
        }
        RowLayout {
          Layout.fillWidth: true
          spacing: Style.space(4)
          Repeater {
            model: ["day", "week", "month", "year", "life"]
            Action {
              required property string modelData
              Layout.fillWidth: true
              text: modelData === "life" ? "Lifetime" : modelData.charAt(0).toUpperCase() + modelData.slice(1)
              checked: root.selectedLens === modelData
              onClicked: root.setLens(modelData)
            }
          }
        }
        Flickable {
          id: scroll
          Layout.fillWidth: true
          Layout.fillHeight: true
          contentWidth: width
          contentHeight: body.implicitHeight
          clip: true
          boundsBehavior: Flickable.StopAtBounds
          Controls.ScrollBar.vertical: Controls.ScrollBar { }
          ColumnLayout {
            id: body
            width: scroll.width - Style.space(12)
            spacing: Style.space(18)
            RowLayout {
              Layout.fillWidth: true
              visible: root.errorText !== "" || root.detailError !== ""
              Label { Layout.fillWidth: true; text: (root.errorText || root.detailError) + (root.errorText && root.panelDataLoaded ? " · Showing the last successful report." : ""); color: Color.urgent }
              Action { text: "Retry"; onClicked: root.refresh() }
            }
            RowLayout {
              Layout.fillWidth: true
              Action { text: "All activity"; visible: root.selected; onClicked: root.selectActivity("", "") }
              Label { Layout.fillWidth: true; text: root.activityLabel; font.pixelSize: Style.font.title; font.bold: true }
              Label { text: root.detailRunning ? "Updating activity…" : ""; color: root.dim }
            }
            GridLayout {
              Layout.fillWidth: true
              columns: root.wide ? 4 : 2
              rowSpacing: Style.space(14)
              columnSpacing: Style.space(18)
              Metric { label: "Foreground time"; value: root.selected && !root.activityDetail ? "…" : Model.fmt(root.shownSeconds); detail: "Selected period" }
              Metric { label: root.selected ? "Days used" : "Recorded time"; value: root.selected ? (root.stats ? String(root.stats.days_used) : "…") : Model.fmt(root.totalObserved); detail: root.selected ? "Days with foreground use" : "Includes recorded inactivity" }
              Metric { label: root.selected ? "Visits" : "Apps used"; value: root.selected ? (root.stats ? String(root.stats.visits) : "…") : String((root.activityAnalytics.activities || []).filter(function(a) { return a.kind === "app" }).length); detail: root.selected ? "Returns within 5 minutes grouped" : "Select one to explore its visits" }
              Metric { label: root.selected ? "Typical visit" : "Gaps in tracking"; value: root.selected ? (root.stats ? Model.fmt(root.stats.median_visit_seconds) : "…") : Model.fmt(root.totalUnobserved); detail: root.selected ? "Time you usually spend each visit" : "Left out when looking for habits" }
            }
            Label {
              Layout.fillWidth: true
              visible: !root.panelDataLoaded || (root.selected && !root.activityDetail) || (root.panelDataLoaded && root.shownSeconds === 0)
              text: !root.panelDataLoaded ? (root.refreshRunning ? "Loading your activity…" : "No report yet. Refresh to try again.")
                : (root.selected && !root.activityDetail ? "Loading this activity’s analytics…" : "No foreground activity recorded in this period.")
              color: root.dim
            }
            GridLayout {
              Layout.fillWidth: true
              columns: root.wide ? 2 : 1
              columnSpacing: Style.space(28)
              rowSpacing: Style.space(22)
              ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: root.wide ? body.width * 0.63 : body.width
                Layout.alignment: Qt.AlignTop
                spacing: Style.space(18)
                GridLayout {
                  Layout.fillWidth: true
                  columns: width >= Style.space(480) ? 2 : 1
                  columnSpacing: Style.space(16)
                  AppDonut {
                    Layout.fillWidth: true
                    Layout.preferredWidth: Style.space(230)
                    apps: root.composition
                    colors: root.compositionColors
                    totalSeconds: root.composition.reduce(function(n, a) { return n + a.seconds }, 0)
                    highlightedIndex: root.composition.findIndex(function(a) { return a.kind === root.selectedActivityKind && a.app_class === root.selectedActivityKey })
                    onActivatedSlice: function(index) { var a = root.composition[index]; root.selectActivity(a.kind, a.app_class) }
                  }
                  ColumnLayout {
                    Layout.fillWidth: true
                    Layout.preferredWidth: Style.space(250)
                    spacing: Style.space(5)
                    Repeater {
                      model: root.composition.slice(0, 5)
                      Action {
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        text: "●  " + modelData.app + " · " + Model.fmt(modelData.seconds) + " · " + modelData.pct + "%"
                        checked: modelData.app_class === root.selectedActivityKey && modelData.kind === root.selectedActivityKind
                        onClicked: root.selectActivity(modelData.kind, modelData.app_class)
                        contentItem: Label { text: parent.text; color: root.colorFromHex(root.compositionColors[index], 1); font.pixelSize: Style.font.caption }
                      }
                    }
                    Label { Layout.fillWidth: true; visible: root.composition.length > 5; text: "+" + (root.composition.length - 5) + " more in Explore activity"; color: root.dim; font.pixelSize: Style.font.caption }
                    Label { Layout.fillWidth: true; visible: root.composition.length === 0; text: root.activityType === "domain" ? "No website activity recorded in this period." : "Your time distribution will appear here."; color: root.dim }
                  }
                }
                FocusRing {
                  Layout.fillWidth: true
                  Layout.preferredHeight: implicitHeight
                  visible: root.selectedLens === "day"
                  title: "Your daily rhythm"
                  detail: "Foreground time · 24 hours"
                  hours: root.hours
                  maxSeconds: root.maximum(root.hours)
                  expanded: true
                }
                FocusTrendLine {
                  Layout.fillWidth: true
                  Layout.preferredHeight: implicitHeight
                  visible: root.selectedLens !== "day"
                  title: root.selectedLens === "life" ? "Recent 13 weeks" : "Usage over time"
                  detail: root.activityLabel
                  days: root.lineDays
                  maxSeconds: root.maximum(root.lineDays)
                  expanded: true
                  onActivatedCell: function(cell) { root.chartReadout = Model.trendDetailText(cell) }
                }
                MonthRhythm {
                  Layout.fillWidth: true
                  Layout.preferredHeight: implicitHeight
                  visible: root.selectedLens === "month"
                  title: "Your month at a glance"
                  detail: "Daily foreground time"
                  cells: root.calendarCells
                  weeks: root.calendarWeeks
                  weekdays: root.calendarWeekdays
                  maxSeconds: root.maximum(root.calendarCells)
                  weekMaxSeconds: root.maximum(root.calendarWeeks)
                  weekdayMaxSeconds: root.maximum(root.calendarWeekdays)
                  onActivatedCell: function(cell) { root.chartReadout = Model.monthCellDetailText(cell) }
                }
                ColumnLayout {
                  Layout.fillWidth: true
                  visible: root.selectedLens === "day"
                  spacing: Style.space(8)
                  Label { text: "Hour by hour"; font.bold: true }
                  BarChart { Layout.fillWidth: true; Layout.minimumHeight: Style.space(175); Layout.preferredHeight: Style.space(175); cells: root.hours; hourly: true }
                }
                Label { Layout.fillWidth: true; visible: root.chartReadout.length > 0; text: root.chartReadout; color: root.dim; font.pixelSize: Style.font.caption }
                ColumnLayout {
                  Layout.fillWidth: true
                  visible: root.selectedLens !== "day"
                  spacing: Style.space(8)
                  Label { text: "When you use it"; font.bold: true }
                  Label { text: "Selected period · weekday and hour totals"; color: root.dim; font.pixelSize: Style.font.caption }
                  Heatmap { Layout.fillWidth: true }
                }
                Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: root.line }
                RowLayout {
                  Layout.fillWidth: true
                  Label { Layout.fillWidth: true; text: "Explore activity"; font.bold: true }
                  Action { text: "Apps"; checked: root.activityType === "app"; onClicked: { root.activityType = "app"; root.showAllActivities = false } }
                  Action { text: "Websites"; checked: root.activityType === "domain"; onClicked: { root.activityType = "domain"; root.showAllActivities = false } }
                }
                Controls.TextField {
                  id: search
                  Layout.fillWidth: true
                  placeholderText: root.activityType === "app" ? "Search apps…" : "Search websites…"
                  color: root.foreground
                  placeholderTextColor: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  Accessible.name: "Search recorded activity"
                  selectByMouse: true
                  padding: Style.space(10)
                  background: Rectangle { color: root.fill; border.width: 1; border.color: search.activeFocus ? root.accent : root.line; radius: Style.space(4) }
                  onTextChanged: root.showAllActivities = false
                }
                Label {
                  Layout.fillWidth: true
                  visible: root.activityType === "domain"
                  text: root.activityAnalytics.browser_domains_enabled === false ? "Website tracking is disabled in your privacy settings."
                    : "Website time is part of browser time. " + Model.fmt(root.activityAnalytics.unattributed_browser_seconds || 0) + " of browser use has no captured domain."
                  color: root.dim
                  font.pixelSize: Style.font.caption
                }
                ColumnLayout {
                  Layout.fillWidth: true
                  spacing: 0
                  Repeater {
                    model: root.showAllActivities || search.text.length ? root.filteredActivities : root.filteredActivities.slice(0, 8)
                    Controls.AbstractButton {
                      id: activityButton
                      required property var modelData
                      Layout.fillWidth: true
                      implicitHeight: activityContent.implicitHeight + Style.space(22)
                      activeFocusOnTab: true
                      Accessible.name: modelData.label + ", " + Model.fmt(modelData.focused_seconds) + ", " + modelData.visits + " visits"
                      onClicked: root.selectActivity(modelData.kind, modelData.key)
                      background: Rectangle { color: activityButton.hovered || activityButton.activeFocus || root.selectedActivityKey === activityButton.modelData.key ? root.fill : "transparent"; border.width: activityButton.activeFocus ? 1 : 0; border.color: root.accent }
                      contentItem: ColumnLayout {
                        id: activityContent
                        spacing: Style.space(5)
                        RowLayout {
                          Layout.fillWidth: true
                          Label { Layout.fillWidth: true; text: activityButton.modelData.label; font.bold: root.selectedActivityKey === activityButton.modelData.key }
                          Label { text: Model.fmt(activityButton.modelData.focused_seconds) }
                        }
                        RowLayout {
                          Layout.fillWidth: true
                          Label { Layout.fillWidth: true; text: activityButton.modelData.days_used + " days · " + activityButton.modelData.visits + " visits"; color: root.dim; font.pixelSize: Style.font.caption }
                          Label { text: Model.fmt(activityButton.modelData.median_visit_seconds) + " typical"; color: root.dim; font.pixelSize: Style.font.caption }
                        }
                        Rectangle {
                          Layout.fillWidth: true
                          implicitHeight: Style.space(3)
                          color: root.fill
                          Rectangle { width: parent.width * Math.min(1, Number(activityButton.modelData.focused_seconds) / Math.max(1, root.totalFocused)); height: parent.height; color: root.accent; opacity: 0.7 }
                        }
                      }
                      leftPadding: Style.space(8)
                      rightPadding: Style.space(8)
                    }
                  }
                }
                Label { Layout.fillWidth: true; visible: root.filteredActivities.length === 0; text: search.text ? "No matching activity." : (root.activityType === "domain" ? "No websites captured in this period. The browser extension supplies domain data." : "No apps recorded in this period."); color: root.dim }
                Action { text: root.showAllActivities ? "Show fewer" : "Show all " + root.filteredActivities.length; visible: root.filteredActivities.length > 8 && !search.text.length; onClicked: root.showAllActivities = !root.showAllActivities }
              }
              ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: root.wide ? body.width * 0.34 : body.width
                Layout.alignment: Qt.AlignTop
                spacing: Style.space(12)
                Label { text: "Patterns & perspective"; font.bold: true }
                Label { Layout.fillWidth: true; text: "Looking back: " + root.baselineText; color: root.dim; font.pixelSize: Style.font.caption }
                Label {
                  Layout.fillWidth: true
                  visible: root.insights.filter(function(i) { return i.kind === "app-routine" }).length === 0
                  text: "Still getting to know your habits. We look for things you do on several days, or return to week after week."
                  color: root.dim
                }
                Repeater {
                  model: root.showAllInsights ? root.insights : root.insights.slice(0, 3)
                  Controls.AbstractButton {
                    id: insightButton
                    required property var modelData
                    readonly property string identityKey: String(modelData.title || modelData.label) + String(modelData.value)
                    readonly property bool expanded: root.expandedInsight === identityKey
                    Layout.fillWidth: true
                    implicitHeight: insightBody.implicitHeight + Style.space(24)
                    activeFocusOnTab: true
                    Accessible.name: String(modelData.title || modelData.label) + ". " + modelData.value + ". See how we know"
                    onClicked: root.inspectInsight(modelData)
                    leftPadding: Style.space(14)
                    rightPadding: Style.space(14)
                    topPadding: Style.space(12)
                    bottomPadding: Style.space(12)
                    background: Rectangle { color: root.fill; radius: Style.space(4); border.width: insightButton.activeFocus ? 1 : 0; border.color: root.accent }
                    contentItem: ColumnLayout {
                      id: insightBody
                      spacing: Style.space(8)
                      Label { Layout.fillWidth: true; text: insightButton.modelData.title || insightButton.modelData.label || "Insight"; font.bold: true }
                      Label { Layout.fillWidth: true; text: insightButton.modelData.value || ""; color: root.accent }
                      Label { Layout.fillWidth: true; text: insightButton.expanded ? root.evidenceText(insightButton.modelData) : String(insightButton.modelData.explanation || insightButton.modelData.detail || ""); color: root.dim; font.pixelSize: Style.font.caption }
                      Label { text: insightButton.expanded ? "Hide details ↑" : "How we know →"; color: root.dim; font.pixelSize: Style.font.caption }
                    }
                  }
                }
                Action { visible: root.insights.length > 3; text: root.showAllInsights ? "Show fewer insights" : "More insights (" + (root.insights.length - 3) + ")"; onClicked: root.showAllInsights = !root.showAllInsights }
                Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: root.line }
                Label { text: "About these numbers"; font.bold: true }
                Label { Layout.fillWidth: true; text: "App time counts the app you were actually using. Time when your computer was idle, locked, asleep, or not being tracked is left out. Coming back within five minutes counts as the same visit, but the time away is never added."; color: root.dim; font.pixelSize: Style.font.caption }
                Label { Layout.fillWidth: true; text: "Recorded pauses · " + Model.fmt(root.totalIdle) + " idle · " + Model.fmt(root.totalLocked) + " locked · " + Model.fmt(root.totalSleep) + " asleep"; color: root.dim; font.pixelSize: Style.font.caption }
                Label { text: root.updatedText ? "Updated " + root.updatedText : "Waiting for a report"; color: root.dim; font.pixelSize: Style.font.caption }
              }
            }
          }
        }
      }
    }
  }

  component Label: Text {
    color: root.foreground
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
    wrapMode: Text.WordWrap
    textFormat: Text.PlainText
  }
  component Action: Controls.AbstractButton {
    id: action
    implicitWidth: Math.max(Style.space(36), contentItem.implicitWidth + Style.space(22))
    implicitHeight: Style.space(36)
    activeFocusOnTab: true
    Accessible.name: text
    opacity: enabled ? 1 : 0.4
    contentItem: Label { text: action.text; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter; font.bold: action.checked }
    background: Rectangle { radius: Style.space(4); color: action.checked || action.hovered ? root.fill : "transparent"; border.width: action.checked || action.activeFocus ? 1 : 0; border.color: action.activeFocus ? root.accent : root.line }
  }
  component Metric: ColumnLayout {
    property string label: ""
    property string value: ""
    property string detail: ""
    Layout.fillWidth: true
    Layout.preferredWidth: 1
    spacing: Style.space(4)
    Label { Layout.fillWidth: true; text: parent.label; color: root.dim; font.pixelSize: Style.font.caption }
    Label { Layout.fillWidth: true; text: parent.value; font.pixelSize: Style.font.title; font.bold: true }
    Label { Layout.fillWidth: true; text: parent.detail; color: root.dim; font.pixelSize: Style.font.caption }
  }
  component BarChart: RowLayout {
    id: chart
    property var cells: []
    property bool hourly: false
    property int inspected: -1
    readonly property real maxSeconds: root.maximum(cells)
    implicitHeight: Style.space(175)
    spacing: Style.space(3)
    Repeater {
      model: chart.cells
      Controls.AbstractButton {
        id: barButton
        required property var modelData
        required property int index
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.preferredWidth: 1
        activeFocusOnTab: true
        readonly property real seconds: Number(modelData.seconds || modelData.focused_seconds || 0)
        readonly property string name: chart.hourly ? String(index).padStart(2, "0") + ":00" : String(modelData.label || modelData.date || "")
        Accessible.name: name + ", " + Model.fmt(seconds)
        onClicked: { chart.inspected = index; root.chartReadout = name + " · " + Model.fmt(seconds) }
        background: Rectangle { color: "transparent"; border.width: barButton.activeFocus ? 1 : 0; border.color: root.accent }
        contentItem: Item {
          Rectangle { anchors.left: parent.left; anchors.right: parent.right; anchors.bottom: barLabel.top; anchors.bottomMargin: Style.space(6); height: Math.max(barButton.seconds > 0 ? Style.space(2) : 0, (parent.height - Style.space(28)) * barButton.seconds / chart.maxSeconds); color: root.accent; opacity: barButton.hovered || chart.inspected === barButton.index ? 1 : 0.65 }
          Label { id: barLabel; anchors.bottom: parent.bottom; width: parent.width; text: chart.hourly ? (barButton.index % 6 === 0 ? String(barButton.index) : "") : (chart.cells.length <= 12 || barButton.index % Math.ceil(chart.cells.length / 6) === 0 ? barButton.name : ""); font.pixelSize: Style.font.caption; color: root.dim; horizontalAlignment: Text.AlignHCenter; elide: Text.ElideRight; wrapMode: Text.NoWrap }
        }
        Controls.ToolTip.visible: hovered
        Controls.ToolTip.text: Accessible.name
      }
    }
  }
  component Heatmap: ColumnLayout {
    spacing: Style.space(4)
    RowLayout {
      Layout.fillWidth: true
      Label { text: ""; Layout.preferredWidth: Style.space(32) }
      Repeater { model: ["00", "06", "12", "18"]; Label { required property string modelData; Layout.fillWidth: true; text: modelData; color: root.dim; font.pixelSize: Style.font.caption } }
    }
    Repeater {
      model: 7
      RowLayout {
        id: heatRow
        required property int index
        property int weekday: index
        Layout.fillWidth: true
        spacing: Style.space(3)
        Label { text: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][parent.weekday]; Layout.preferredWidth: Style.space(32); font.pixelSize: Style.font.caption; color: root.dim }
        Repeater {
          model: 24
          Controls.AbstractButton {
            id: cell
            required property int index
            readonly property int weekday: heatRow.weekday
            readonly property var datum: root.heatCells[weekday * 24 + index] || {}
            readonly property real seconds: Number(datum.seconds || datum.focused_seconds || 0)
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            implicitHeight: Style.space(22)
            activeFocusOnTab: true
            Accessible.name: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][weekday] + " " + index + ":00 · " + Model.fmt(seconds)
            onClicked: root.chartReadout = Accessible.name
            background: Rectangle { radius: Style.space(2); color: cell.seconds > 0 ? root.accent : root.fill; opacity: cell.seconds > 0 ? 0.2 + 0.8 * cell.seconds / root.maximum(root.heatCells) : 1; border.width: cell.activeFocus ? 2 : 0; border.color: root.foreground }
            Controls.ToolTip.visible: hovered
            Controls.ToolTip.text: Accessible.name
          }
        }
      }
    }
  }
  component AppDonut: Item {
    id: donutRoot

    property var apps: []
    property var colors: []
    property real totalSeconds: 0
    property int hoveredIndex: -1
    property int highlightedIndex: -1
    signal highlightChanged(int index)
    signal activatedSlice(int index)

    readonly property var segments: Model.arcSegments(apps)
    readonly property int activeIndex: hoveredIndex >= 0 ? hoveredIndex : highlightedIndex
    readonly property bool hasActiveApp: activeIndex >= 0 && activeIndex < apps.length
    readonly property string centerLabel: hasActiveApp ? root.formatDuration(Number(apps[activeIndex].seconds || 0)) : root.formatDuration(totalSeconds)
    readonly property string centerDetail: hasActiveApp ? String(apps[activeIndex].app || "App") : "Foreground"
    readonly property int chartSize: Math.min(Style.space(190), Math.max(Style.space(142), width - Style.space(24)))

    Layout.minimumHeight: Style.space(252)
    implicitHeight: Style.space(252)

    onAppsChanged: donutCanvas.requestPaint()
    onColorsChanged: donutCanvas.requestPaint()
    onHoveredIndexChanged: {
      donutCanvas.requestPaint()
      highlightChanged(hoveredIndex)
    }
    onHighlightedIndexChanged: donutCanvas.requestPaint()
    onWidthChanged: donutCanvas.requestPaint()

    Column {
      anchors.fill: parent
      anchors.margins: Style.space(12)
      spacing: Style.space(8)

      Item {
        width: parent.width
        height: Style.space(20)

        Text {
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: root.activityType === "app" ? "Time by app" : "Time by website"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          font.bold: true
          elide: Text.ElideRight
        }
      }

      Item {
        width: parent.width
        height: donutRoot.chartSize

        Canvas {
          id: donutCanvas

          anchors.centerIn: parent
          width: donutRoot.chartSize
          height: donutRoot.chartSize
          antialiasing: true

          onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            var cx = width / 2
            var cy = height / 2
            var radius = Math.min(width, height) / 2 - Style.space(13)
            var lineWidth = Math.max(Style.space(13), radius * 0.18)
            ctx.lineCap = "butt"

            ctx.beginPath()
            ctx.arc(cx, cy, radius, 0, Math.PI * 2, false)
            ctx.strokeStyle = root.canvasColor(root.foreground, 0.12)
            ctx.lineWidth = lineWidth
            ctx.stroke()

            for (var i = 0; i < donutRoot.segments.length; i++) {
              var segment = donutRoot.segments[i]
              if (Number(segment.sweepAngle || 0) <= 0) continue
              var start = Number(segment.startAngle || 0) * Math.PI / 180
              var end = Number(segment.startAngle + segment.sweepAngle) * Math.PI / 180
              var active = i === donutRoot.activeIndex
              var color = root.colorFromHex(String(donutRoot.colors[i] || Color.accent), active || donutRoot.activeIndex < 0 ? 0.94 : 0.46)
              ctx.beginPath()
              ctx.arc(cx, cy, radius, start, end, false)
              ctx.strokeStyle = root.canvasColor(color, color.a)
              ctx.lineWidth = active ? lineWidth + Style.space(3) : lineWidth
              ctx.stroke()
            }
          }
        }

        Column {
          width: parent.width * 0.66
          anchors.centerIn: parent
          spacing: Style.space(1)

          Text {
            width: parent.width
            text: donutRoot.centerLabel
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            font.bold: true
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
          }

          Text {
            width: parent.width
            text: donutRoot.centerDetail
            color: root.faint
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
          }
        }

        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          onPositionChanged: function(mouse) {
            var cx = width / 2
            var cy = height / 2
            var dx = mouse.x - cx
            var dy = mouse.y - cy
            var distance = Math.sqrt(dx * dx + dy * dy)
            var outer = donutRoot.chartSize / 2
            var inner = outer - Style.space(46)
            if (distance < inner || distance > outer) {
              donutRoot.hoveredIndex = -1
              return
            }
            var angle = Math.atan2(dy, dx) * 180 / Math.PI
            for (var i = 0; i < donutRoot.segments.length; i++) {
              var segment = donutRoot.segments[i]
              var start = Number(segment.startAngle || 0)
              var sweep = Number(segment.sweepAngle || 0)
              var normalized = angle
              while (normalized < start) normalized += 360
              if (normalized >= start && normalized <= start + sweep) {
                donutRoot.hoveredIndex = i
                return
              }
            }
            donutRoot.hoveredIndex = -1
          }
          onClicked: { if (donutRoot.hoveredIndex >= 0) donutRoot.activatedSlice(donutRoot.hoveredIndex) }
          onExited: {
            donutRoot.hoveredIndex = -1
            donutRoot.highlightChanged(-1)
          }
        }
      }

      Text {
        width: parent.width
        text: donutRoot.hasActiveApp
          ? String(donutRoot.apps[donutRoot.activeIndex].app || "App") + "  " + root.formatDuration(Number(donutRoot.apps[donutRoot.activeIndex].seconds || 0)) + "  " + Number(donutRoot.apps[donutRoot.activeIndex].pct || 0) + "%"
          : "Select a slice to explore"
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
      }
    }
  }

  component FocusRing: Rectangle {
    id: ringRoot

    property string title: ""
    property string detail: ""
    property var hours: []
    property real maxSeconds: 0
    property int selectedIndex: -1
    property int hoveredIndex: -1
    property string hoveredText: ""
    property bool expanded: false
    readonly property int chartSize: expanded ? Style.space(146) : Style.space(132)
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < hours.length
      ? hourlyDetailText(hours[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property var peak: Model.bestHour(hours)
    readonly property string defaultText: peak.label !== "--" ? "Peak " + peak.label + ": " + peak.value + "  " + peak.detail : ""

    function hourlyDetailText(cell) {
      if (!cell) return ""
      return String(cell.fullLabel || cell.label || "Hour") + ": " + root.formatDuration(Number(cell.seconds || 0)) + " focused"
    }

    function segmentIndexAt(px, py) {
      var cx = ringCanvas.width / 2
      var cy = ringCanvas.height / 2
      var dx = px - ringCanvas.x - cx
      var dy = py - ringCanvas.y - cy
      var distance = Math.sqrt(dx * dx + dy * dy)
      var outer = Math.min(ringCanvas.width, ringCanvas.height) / 2
      var inner = outer - Style.space(48)
      if (distance < inner || distance > outer) return -1
      var degrees = Math.atan2(dy, dx) * 180 / Math.PI + 90
      while (degrees < 0) degrees += 360
      return Math.max(0, Math.min(23, Math.floor(degrees / 15)))
    }

    implicitHeight: expanded ? Style.space(216) : Style.space(184)
    radius: 0
    color: root.noFill
    border.width: 0

    onHoursChanged: ringCanvas.requestPaint()
    onMaxSecondsChanged: ringCanvas.requestPaint()
    onSelectedIndexChanged: ringCanvas.requestPaint()
    onHoveredIndexChanged: ringCanvas.requestPaint()

    Item {
      id: ringHeader

      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(12)
      height: Style.space(20)

      Text {
        anchors.left: parent.left
        anchors.right: ringDetail.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: ringRoot.title
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: ringDetail

        width: Math.min(implicitWidth, parent.width * 0.46)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: ringRoot.detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
      }
    }

    Item {
      id: ringBody
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: ringHeader.bottom
      anchors.bottom: ringReadout.top
      anchors.margins: Style.space(12)
      anchors.topMargin: Style.space(8)
      anchors.bottomMargin: Style.space(8)

      Canvas {
        id: ringCanvas

        width: ringRoot.chartSize
        height: ringRoot.chartSize
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        antialiasing: true

        onPaint: {
          var ctx = getContext("2d")
          ctx.reset()
          var cx = width / 2
          var cy = height / 2
          var radius = Math.min(width, height) / 2 - Style.space(11)
          var trackWidth = Style.space(10)
          var activeWidth = Style.space(15)
          ctx.lineCap = "round"

          for (var i = 0; i < 24; i++) {
            var start = (-90 + i * 15 + 1.7) * Math.PI / 180
            var end = (-90 + (i + 1) * 15 - 1.7) * Math.PI / 180
            ctx.beginPath()
            ctx.arc(cx, cy, radius, start, end, false)
            ctx.strokeStyle = root.canvasColor(root.foreground, 0.10)
            ctx.lineWidth = trackWidth
            ctx.stroke()
          }

          for (var h = 0; h < Math.min(24, ringRoot.hours.length); h++) {
            var seconds = Number(ringRoot.hours[h].seconds || 0)
            if (seconds <= 0 || ringRoot.maxSeconds <= 0) continue
            var intensity = root.clamp01(seconds / ringRoot.maxSeconds)
            var active = h === ringRoot.hoveredIndex || h === ringRoot.selectedIndex
            var startAngle = (-90 + h * 15 + 1.7) * Math.PI / 180
            var endAngle = (-90 + (h + 1) * 15 - 1.7) * Math.PI / 180
            ctx.beginPath()
            ctx.arc(cx, cy, radius, startAngle, endAngle, false)
            ctx.strokeStyle = root.canvasColor(root.sliceColor(0, 1.0), active ? 1.0 : 0.36 + intensity * 0.54)
            ctx.lineWidth = active ? activeWidth + Style.space(3) : activeWidth
            ctx.stroke()
          }
        }
      }

      Column {
        anchors.left: ringCanvas.right
        anchors.leftMargin: Style.space(14)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(8)

        Text {
          width: parent.width
          text: ringRoot.peak.label
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.title
          font.bold: true
          elide: Text.ElideRight
        }

        Text {
          width: parent.width
          text: ringRoot.peak.value + " peak focus"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          font.bold: true
          elide: Text.ElideRight
        }

        Text {
          width: parent.width
          text: ringRoot.peak.detail
          color: root.faint
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onPositionChanged: function(mouse) {
          var index = ringRoot.segmentIndexAt(mouse.x, mouse.y)
          ringRoot.hoveredIndex = index
          ringRoot.hoveredText = index >= 0 && index < ringRoot.hours.length ? ringRoot.hourlyDetailText(ringRoot.hours[index]) : ""
        }
        onExited: {
          ringRoot.hoveredIndex = -1
          ringRoot.hoveredText = ""
        }
      }
    }

    ChartReadout {
      id: ringReadout
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.margins: Style.space(10)
      text: ringRoot.readoutText.length > 0 ? ringRoot.readoutText : ringRoot.defaultText
    }

    Rectangle {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      height: 1
      color: root.hairline
    }
  }

  component FocusTrendLine: Rectangle {
    id: lineRoot

    property string title: ""
    property string detail: ""
    property var days: []
    property real maxSeconds: 0
    property int selectedIndex: -1
    property int hoveredIndex: -1
    property string hoveredText: ""
    property bool expanded: false
    signal activatedCell(var cell)
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < days.length
      ? Model.trendDetailText(days[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property string defaultText: Model.trendDefaultText(days)
    readonly property real averageSeconds: {
      var list = days || []
      if (list.length <= 0) return 0
      var total = 0
      for (var i = 0; i < list.length; i++) total += Number(list[i].seconds || 0)
      return total / list.length
    }

    function pointX(index) {
      var count = Math.max(1, days.length)
      if (count === 1) return linePlot.width / 2
      return index * linePlot.width / (count - 1)
    }

    function pointY(seconds) {
      var value = root.clamp01(Number(seconds || 0) / Math.max(1, lineRoot.maxSeconds))
      return Math.max(0, Math.min(linePlot.height, linePlot.height - value * linePlot.height))
    }

    function indexAt(x) {
      var count = days.length
      if (count <= 0 || linePlot.width <= 0) return -1
      if (count === 1) return 0
      return Math.max(0, Math.min(count - 1, Math.round(x / linePlot.width * (count - 1))))
    }

    function requestPaint() {
      if (lineCanvas) lineCanvas.requestPaint()
    }

    implicitHeight: expanded ? Style.space(216) : Style.space(184)
    radius: 0
    color: root.noFill
    border.width: 0

    onDaysChanged: requestPaint()
    onMaxSecondsChanged: requestPaint()
    onHoveredIndexChanged: requestPaint()
    onSelectedIndexChanged: requestPaint()
    onAverageSecondsChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    Item {
      id: lineHeader

      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(12)
      height: Style.space(20)

      Text {
        anchors.left: parent.left
        anchors.right: lineDetail.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: lineRoot.title
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: lineDetail

        width: Math.min(implicitWidth, parent.width * 0.46)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: lineRoot.detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
      }
    }

    Item {
      id: linePlot
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: lineHeader.bottom
      anchors.bottom: lineReadout.top
      anchors.margins: Style.space(12)
      anchors.topMargin: Style.space(12)
      anchors.bottomMargin: Style.space(26)

      Repeater {
        model: 3

        Rectangle {
          required property int index

          anchors.left: parent.left
          anchors.right: parent.right
          y: Math.round((index + 1) * parent.height / 4)
          height: 1
          color: root.line
          opacity: 0.32
        }
      }

      Rectangle {
        visible: lineRoot.maxSeconds > 0 && lineRoot.averageSeconds > 0
        anchors.left: parent.left
        anchors.right: parent.right
        y: Math.round(lineRoot.pointY(lineRoot.averageSeconds))
        height: 1
        color: root.withAlpha(root.foreground, 0.26)
      }

      Canvas {
        id: lineCanvas

        anchors.fill: parent
        antialiasing: true

        onPaint: {
          var ctx = getContext("2d")
          ctx.reset()
          var list = lineRoot.days || []
          if (list.length <= 0 || lineRoot.maxSeconds <= 0 || width <= 0 || height <= 0) return

          var accent = root.sliceColor(0, 1.0)
          var mutedAccent = root.sliceColor(0, 0.26)
          var area = ctx.createLinearGradient(0, 0, 0, height)
          area.addColorStop(0, root.canvasColor(accent, 0.28))
          area.addColorStop(1, root.canvasColor(accent, 0.03))

          ctx.beginPath()
          ctx.moveTo(lineRoot.pointX(0), height)
          for (var i = 0; i < list.length; i++) {
            ctx.lineTo(lineRoot.pointX(i), lineRoot.pointY(Number(list[i].seconds || 0)))
          }
          ctx.lineTo(lineRoot.pointX(list.length - 1), height)
          ctx.closePath()
          ctx.fillStyle = area
          ctx.fill()

          ctx.beginPath()
          for (var j = 0; j < list.length; j++) {
            var x = lineRoot.pointX(j)
            var y = lineRoot.pointY(Number(list[j].seconds || 0))
            if (j === 0) ctx.moveTo(x, y)
            else ctx.lineTo(x, y)
          }
          ctx.strokeStyle = root.canvasColor(accent, 0.92)
          ctx.lineWidth = Style.space(3)
          ctx.lineJoin = "round"
          ctx.lineCap = "round"
          ctx.stroke()

          var count = list.length
          var dotStride = count <= 14 ? 1 : (count <= 31 ? 3 : Math.ceil(count / 12))
          for (var k = 0; k < count; k++) {
            var seconds = Number(list[k].seconds || 0)
            var active = k === lineRoot.hoveredIndex || k === lineRoot.selectedIndex
            if (!active && seconds <= 0 && k % dotStride !== 0) continue
            if (!active && k % dotStride !== 0 && k !== 0 && k !== count - 1) continue
            var px = lineRoot.pointX(k)
            var py = lineRoot.pointY(seconds)
            ctx.beginPath()
            ctx.arc(px, py, active ? Style.space(5) : Style.space(3), 0, Math.PI * 2, false)
            ctx.fillStyle = active ? root.canvasColor(root.foreground, 0.96) : root.canvasColor(mutedAccent, mutedAccent.a)
            ctx.fill()
            ctx.lineWidth = active ? Style.space(2) : 0
            if (active) {
              ctx.strokeStyle = root.canvasColor(accent, 0.95)
              ctx.stroke()
            }
          }
        }
      }

      Rectangle {
        visible: lineRoot.hoveredIndex >= 0 || lineRoot.selectedIndex >= 0
        x: {
          var index = lineRoot.hoveredIndex >= 0 ? lineRoot.hoveredIndex : lineRoot.selectedIndex
          return Math.max(0, Math.min(parent.width - width, lineRoot.pointX(index) - width / 2))
        }
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: 1
        color: root.withAlpha(root.foreground, 0.24)
      }

      Text {
        anchors.left: parent.left
        anchors.top: parent.bottom
        anchors.topMargin: Style.space(5)
        text: days.length > 0 ? String(days[0].label || days[0].fullLabel || "") : ""
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }

      Text {
        width: parent.width * 0.42
        anchors.right: parent.right
        anchors.top: parent.bottom
        anchors.topMargin: Style.space(5)
        text: days.length > 0 ? String(days[days.length - 1].label || days[days.length - 1].fullLabel || "") : ""
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
      }

      MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onPositionChanged: function(mouse) {
          var index = lineRoot.indexAt(mouse.x)
          lineRoot.hoveredIndex = index
          lineRoot.hoveredText = index >= 0 && index < lineRoot.days.length ? Model.trendDetailText(lineRoot.days[index]) : ""
        }
        onClicked: function(mouse) {
          var index = lineRoot.indexAt(mouse.x)
          if (index >= 0 && index < lineRoot.days.length) lineRoot.activatedCell(lineRoot.days[index])
        }
        onExited: {
          lineRoot.hoveredIndex = -1
          lineRoot.hoveredText = ""
        }
      }
    }

    ChartReadout {
      id: lineReadout
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.margins: Style.space(10)
      text: lineRoot.readoutText.length > 0 ? lineRoot.readoutText : lineRoot.defaultText
    }

    Rectangle {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      height: 1
      color: root.hairline
    }
  }

  component MonthRhythm: Rectangle {
    id: monthRhythmRoot

    property string title: ""
    property string detail: ""
    property var cells: []
    property var weeks: []
    property var weekdays: []
    property real maxSeconds: 0
    property real weekMaxSeconds: 0
    property real weekdayMaxSeconds: 0
    property int selectedIndex: -1
    property int hoveredIndex: -1
    property string hoveredText: ""
    property real revealProgress: 0
    signal activatedCell(var cell)
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < cells.length
      ? Model.monthCellDetailText(cells[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property string defaultText: Model.monthDefaultText(cells, false)
    readonly property bool compact: width > 0 && width < Style.space(540)
    readonly property real gap: Style.space(4)
    readonly property int rowCount: Math.ceil(cells.length / 7)
    readonly property real cellSize: Math.max(Style.space(22), Math.min(Style.space(32), (width - Style.space(compact ? 28 : 304)) / 7))
    readonly property real calendarWidth: 7 * cellSize + 6 * gap
    readonly property real calendarHeight: Style.space(18) + Style.space(4) + rowCount * cellSize + Math.max(0, rowCount - 1) * gap
    readonly property real weeklyPaceHeight: Style.space(18) + Style.space(5) + weeks.length * Style.space(22) + Math.max(0, weeks.length - 1) * Style.space(5)
    readonly property real sideHeight: weeklyPaceHeight + Style.space(10) + Style.space(54)
    readonly property real bodyHeight: compact ? calendarHeight + Style.space(10) + sideHeight : Math.max(calendarHeight, sideHeight)

    function restartReveal() {
      revealProgress = 0
      monthRhythmReveal.restart()
    }

    onCellsChanged: restartReveal()
    onMaxSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: Style.space(12) + monthRhythmHeader.height + Style.space(10) + bodyHeight + Style.space(10) + monthRhythmReadout.height + Style.space(10)
    radius: 0
    color: root.noFill
    border.width: 0

    NumberAnimation {
      id: monthRhythmReveal

      target: monthRhythmRoot
      property: "revealProgress"
      from: 0
      to: 1
      duration: 620
      easing.type: Easing.OutCubic
    }

    Item {
      id: monthRhythmHeader

      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(12)
      height: Style.space(20)

      Text {
        anchors.left: parent.left
        anchors.right: monthRhythmDetail.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: monthRhythmRoot.title
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: monthRhythmDetail

        width: Math.min(implicitWidth, parent.width * 0.46)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: monthRhythmRoot.detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
      }
    }

    GridLayout {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: monthRhythmHeader.bottom
      anchors.bottom: monthRhythmReadout.top
      anchors.margins: Style.space(12)
      anchors.topMargin: Style.space(10)
      anchors.bottomMargin: Style.space(10)
      columns: monthRhythmRoot.compact ? 1 : 2
      columnSpacing: Style.space(14)
      rowSpacing: Style.space(10)

      Column {
        Layout.preferredWidth: monthRhythmRoot.compact ? parent.width : monthRhythmRoot.calendarWidth
        Layout.alignment: Qt.AlignTop
        spacing: Style.space(4)

        Row {
          width: monthRhythmRoot.calendarWidth
          spacing: monthRhythmRoot.gap

          Repeater {
            model: Model.weekdayLabels()

            Text {
              required property string modelData
              width: monthRhythmRoot.cellSize
              text: modelData.substr(0, 1)
              color: root.faint
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              horizontalAlignment: Text.AlignHCenter
            }
          }
        }

        Grid {
          width: monthRhythmRoot.calendarWidth
          columns: 7
          rowSpacing: monthRhythmRoot.gap
          columnSpacing: monthRhythmRoot.gap

          Repeater {
            model: cells

            Rectangle {
              required property int index
              required property var modelData
              readonly property real cellSeconds: Number(modelData.seconds || 0)
              readonly property real cellIntensity: Model.heatIntensity(cellSeconds, monthRhythmRoot.maxSeconds)
              readonly property color heatBase: root.sliceColor(0, 1.0)

              width: monthRhythmRoot.cellSize
              height: width
              radius: Style.space(4)
              color: modelData.blank
                ? "transparent"
                : (cellSeconds > 0
                  ? root.withAlpha(heatBase, 0.10 + monthRhythmRoot.revealProgress * (0.14 + 0.70 * cellIntensity))
                  : root.track)
              border.color: modelData.blank ? "transparent" : root.line
              border.width: modelData.blank ? 0 : 1
              scale: (monthRhythmRoot.hoveredIndex === index || monthRhythmRoot.selectedIndex === index) && !modelData.blank ? 1.05 : 1.0

              Behavior on color {
                ColorAnimation { duration: 140 }
              }

              Behavior on scale {
                NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
              }

              Text {
                anchors.centerIn: parent
                text: modelData.blank ? "" : String(modelData.day || "")
                color: cellSeconds > 0 ? root.foreground : root.faint
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                font.bold: cellSeconds > 0
              }

              MouseArea {
                anchors.fill: parent
                enabled: !modelData.blank
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: {
                  monthRhythmRoot.hoveredIndex = index
                  monthRhythmRoot.hoveredText = Model.monthCellDetailText(modelData)
                }
                onExited: {
                  if (monthRhythmRoot.hoveredIndex === index) {
                    monthRhythmRoot.hoveredIndex = -1
                    monthRhythmRoot.hoveredText = ""
                  }
                }
                onClicked: monthRhythmRoot.activatedCell(modelData)
              }
            }
          }
        }
      }

      Column {
        Layout.fillWidth: true
        Layout.alignment: Qt.AlignTop
        spacing: Style.space(10)

        Column {
          width: parent.width
          spacing: Style.space(5)

          Text {
            width: parent.width
            text: "Weekly pace"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            elide: Text.ElideRight
          }

          Repeater {
            model: weeks

            Item {
              required property var modelData

              width: parent.width
              height: Style.space(22)

              Text {
                id: weekName
                anchors.left: parent.left
                anchors.top: parent.top
                width: Style.space(74)
                text: String(modelData.label || "")
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                elide: Text.ElideRight
              }

              Text {
                id: weekValue
                anchors.right: parent.right
                anchors.top: parent.top
                width: Style.space(72)
                text: String(modelData.valueText || "")
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                font.bold: true
                horizontalAlignment: Text.AlignRight
                elide: Text.ElideRight
              }

              Rectangle {
                anchors.left: weekName.right
                anchors.right: weekValue.left
                anchors.leftMargin: Style.space(8)
                anchors.rightMargin: Style.space(8)
                anchors.verticalCenter: weekName.verticalCenter
                height: Style.space(6)
                radius: height / 2
                color: root.track

                Rectangle {
                  width: parent.width * root.clamp01(Number(modelData.seconds || 0) / monthRhythmRoot.weekMaxSeconds) * monthRhythmRoot.revealProgress
                  height: parent.height
                  radius: parent.radius
                  color: root.sliceColor(0, 0.9)
                }
              }
            }
          }
        }

        Item {
          width: parent.width
          height: Style.space(54)

          Text {
            anchors.left: parent.left
            anchors.top: parent.top
            text: "Weekday balance"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }

          Repeater {
            model: weekdays

            Item {
              required property int index
              required property var modelData

              readonly property real itemGap: Style.space(5)
              readonly property real itemWidth: (parent.width - itemGap * 6) / 7

              x: index * (itemWidth + itemGap)
              y: Style.space(18)
              width: itemWidth
              height: Style.space(36)

              Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: dayLabel.top
                anchors.bottomMargin: Style.space(3)
                height: Style.space(16)
                radius: Style.space(3)
                color: root.track

                Rectangle {
                  anchors.left: parent.left
                  anchors.bottom: parent.bottom
                  width: parent.width
                  height: Math.max(Style.space(3), parent.height * root.clamp01(Number(modelData.seconds || 0) / monthRhythmRoot.weekdayMaxSeconds) * monthRhythmRoot.revealProgress)
                  radius: parent.radius
                  color: root.sliceColor(1, 0.82)
                }
              }

              Text {
                id: dayLabel
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                text: String(modelData.label || "").substr(0, 1)
                color: root.faint
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                horizontalAlignment: Text.AlignHCenter
              }
            }
          }
        }
      }
    }

    ChartReadout {
      id: monthRhythmReadout

      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.margins: Style.space(10)
      text: monthRhythmRoot.readoutText.length > 0 ? monthRhythmRoot.readoutText : monthRhythmRoot.defaultText
    }

    Rectangle {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      height: 1
      color: root.hairline
    }
  }

  component ChartReadout: Rectangle {
    id: readoutRoot

    property string text: ""

    visible: text.length > 0
    implicitHeight: visible ? Style.space(24) : 0
    height: visible ? Style.space(24) : 0
    radius: Style.space(5)
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
    border.width: 0
    clip: true
    opacity: visible ? 1 : 0
    scale: visible ? 1 : 0.98

    Behavior on opacity {
      NumberAnimation { duration: 120 }
    }

    Behavior on scale {
      NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
    }

    Text {
      id: readoutLabel

      anchors.fill: parent
      anchors.leftMargin: Style.space(8)
      anchors.rightMargin: Style.space(8)
      text: readoutRoot.text
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      verticalAlignment: Text.AlignVCenter
      elide: Text.ElideRight
    }
  }

}
