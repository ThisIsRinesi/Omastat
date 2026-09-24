import QtQuick
import Quickshell
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
  property Item islandContainer: null
  readonly property bool canEmbedIsland: dynamicIslandStyle && bar && typeof bar.presentIslandPanel === "function"
  function syncIsland() {
    if (!panel) return
    if (opened && canEmbedIsland) {
      islandReturnTimer.stop()
      islandContainer = bar.presentIslandPanel(root, panel.contentWidth, panel.contentHeight,
        anchorItem && anchorItem.QsWindow.window && anchorItem.QsWindow.window.screen
          ? anchorItem.QsWindow.window.screen.name : "", motionDuration === 0)
      Qt.callLater(function() { if (root.opened) keyCatcher.forceActiveFocus() })
    } else {
      if (bar && typeof bar.dismissIslandPanel === "function") bar.dismissIslandPanel(root)
      if (islandContainer) islandReturnTimer.restart()
    }
  }
  Timer {
    id: islandReturnTimer
    interval: root.motionDuration === 0 ? 0 : 240
    onTriggered: root.islandContainer = null
  }
  onCanEmbedIslandChanged: syncIsland()
  Component.onDestruction: if (bar && typeof bar.dismissIslandPanel === "function") bar.dismissIslandPanel(root)
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
  property var multitasking: ({})
  property real totalMultitasked: 0
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
  property var inspectedInsight: null
  property var insightReturnControl: null
  property bool dataExpanded: false
  property string chartReadout: ""
  property int legendHoverIndex: -1
  readonly property bool dynamicIslandStyle: {
    var value = root.setting("dynamicIslandStyle", false)
    return value === true || value === "true"
  }
  property bool appearanceOpen: false
  readonly property bool richGraphs: root.setting("richGraphs", true) !== false && root.setting("richGraphs", true) !== "false"
  readonly property bool reducedMotion: root.setting("reduceMotion", false) === true || root.setting("reduceMotion", false) === "true"
  readonly property int motionDuration: reducedMotion ? 0 : (richGraphs ? 240 : 150)
  property real graphProgress: 1
  function setAppearance(name, value) {
    if (hostWidget && typeof hostWidget.setAppearanceSetting === "function") hostWidget.setAppearanceSetting(name, value)
  }
  function settleGraphMotion() {
    graphEntry.stop()
    graphProgress = 1
  }
  onViewRevealed: {
    if (richGraphs && motionDuration > 0) {
      if (!graphEntry.running) graphEntry.start()
    } else settleGraphMotion()
  }
  NumberAnimation {
    id: graphEntry
    target: root; property: "graphProgress"; from: 0; to: 1
    duration: root.motionDuration === 0 ? 0 : 360
    easing.type: Easing.OutQuint
  }
  onCompositionChanged: legendHoverIndex = -1
  property bool refreshBusyVisible: false
  property bool detailBusyVisible: false
  property bool revealQueued: false
  property string revealedViewKey: ""
  readonly property string viewKey: JSON.stringify([selectedLens, selectedOffset, selectedActivityKind, selectedActivityKey])
  readonly property bool viewReady: panelDataLoaded && (!selected || activityDetail !== null)
  signal viewRevealed()

  onRefreshRunningChanged: updateBusyFeedback()
  onDetailRunningChanged: updateBusyFeedback()
  onViewKeyChanged: scheduleViewReveal()
  onViewReadyChanged: scheduleViewReveal()
  onOpenedChanged: {
    syncIsland()
    updateBusyFeedback()
    if (opened) {
      scheduleViewReveal()
    }
    else {
      revealedViewKey = ""
      appearanceOpen = false
      settleGraphMotion()
      dataReveal.stop()
      body.opacity = 1
    }
  }
  onMotionDurationChanged: if (motionDuration === 0) {
    settleGraphMotion()
    dataReveal.stop()
    body.opacity = 1
  }

  Timer {
    id: refreshFeedbackDelay
    interval: 150
    onTriggered: root.refreshBusyVisible = root.opened && root.refreshRunning
  }
  Timer {
    id: detailFeedbackDelay
    interval: 150
    onTriggered: root.detailBusyVisible = root.opened && root.detailRunning
  }

  function updateBusyFeedback() {
    if (!opened || !refreshRunning) {
      refreshFeedbackDelay.stop()
      refreshBusyVisible = false
    } else if (!refreshBusyVisible && !refreshFeedbackDelay.running) refreshFeedbackDelay.start()
    if (!opened || !detailRunning) {
      detailFeedbackDelay.stop()
      detailBusyVisible = false
    } else if (!detailBusyVisible && !detailFeedbackDelay.running) detailFeedbackDelay.start()
  }

  function scheduleViewReveal() {
    if (revealQueued) return
    revealQueued = true
    // Wait for the controller to finish injecting the entire selected report.
    Qt.callLater(function() {
      revealQueued = false
      if (!opened || !viewReady || revealedViewKey === viewKey) return
      revealedViewKey = viewKey
      if (motionDuration > 0) dataReveal.restart()
      else {
        dataReveal.stop()
        body.opacity = 1
      }
      viewRevealed()
    })
  }
  readonly property color foreground: dynamicIslandStyle ? "#f5f5f7" : (bar ? bar.barForeground : Color.foreground)
  readonly property color accent: Color.accent
  readonly property color dim: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.75)
  readonly property color line: Qt.rgba(foreground.r, foreground.g, foreground.b, dynamicIslandStyle ? 0.12 : 0.18)
  readonly property color fill: Qt.rgba(foreground.r, foreground.g, foreground.b, dynamicIslandStyle ? 0.07 : 0.06)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property int requestedWidth: Math.max(380, Math.min(2000, Number(root.setting("panelWidth", 1160)) || 1160))
  readonly property bool expansive: panel.contentWidth >= Style.space(1500)
  readonly property bool wide: panel.contentWidth >= Style.space(900)
  readonly property bool calendarLens: selectedLens === "month" || selectedLens === "year"
  readonly property bool supportRailVisible: expansive && (hasInsightContent || calendarLens || selectedLens === "day")
  readonly property bool selected: selectedActivityKey.length > 0
  readonly property var detail: selected ? (activityDetail || {}) : activityAnalytics
  readonly property var activityMetrics: Model.activityMetricValues(activityDetail, detailRunning, detailError)
  readonly property var stats: selected && detail.activities && detail.activities.length ? detail.activities[0] : null
  readonly property string activityLabel: stats ? String(stats.label) : (selected ? selectedActivityKey : "All activity")
  readonly property real shownSeconds: selected ? (stats ? Number(stats.focused_seconds) : 0) : totalFocused
  readonly property var shownDaily: selected ? (detail.daily || []) : Model.withMultitaskingDays(daily, multitasking.daily)
  readonly property var shownHeat: selected ? (detail.heatmap || []) : heatmap
  readonly property var hours: Model.withMultitaskingHours(Model.hourlyCells(shownHeat), selected ? [] : multitasking.heatmap)
  readonly property var trend: Model.activityCells(shownDaily, selectedLens)
  readonly property var heatCells: Model.heatmapCells(shownHeat)
  readonly property var insights: Model.widgetInsights(selected ? (detail.insights || []) : reportInsights)
  readonly property var filteredActivities: {
    var list = activityAnalytics.activities || []
    var query = search.text.toLowerCase().trim()
    return list.filter(function(item) {
      return item.kind === root.activityType && (!query || String(item.label).toLowerCase().indexOf(query) >= 0 || String(item.key).toLowerCase().indexOf(query) >= 0)
    })
  }
  readonly property bool hasInsightContent: insights.length > 0 || inspectedInsight !== null
  readonly property string baselineText: detail.baseline_start
    ? Model.insightDateRange(detail.baseline_start, detail.baseline_end) : "Up to eight weeks of history"
  onSelectedLensChanged: resetView()
  onSelectedOffsetChanged: resetView()
  onSelectedActivityKeyChanged: resetView()

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
  readonly property bool yearlyRhythm: selectedLens === "year"
  readonly property var rhythmCells: yearlyRhythm ? Model.monthCells(shownDaily, "year") : calendarCells
  readonly property var rhythmWeeks: yearlyRhythm ? Model.monthBucketCells(shownDaily) : calendarWeeks
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


  function resetView() { dataExpanded = false; chartReadout = ""; inspectedInsight = null; insightReturnControl = null; showAllInsights = false; scroll.contentY = 0 }
  function refresh() { if (hostWidget) { hostWidget.refresh(true); hostWidget.refreshDetail() } }
  function setLens(lens) { if (hostWidget) hostWidget.setPeriod(lens, 0) }
  function setLensOffset(lens, offset) { if (hostWidget) hostWidget.setPeriod(lens, offset) }
  function focusPeriod(lens) {
    Qt.callLater(function() {
      var index = ["day", "week", "month", "year", "life"].indexOf(lens)
      var button = lensButtons.itemAt(index)
      if (button) button.forceActiveFocus()
    })
  }
  function openCell(cell) {
    var destination = Model.cellDestination(cell, Model.dateKey(new Date()))
    if (!destination || !hostWidget) return
    // Release chart focus before changing the period can hide its delegate.
    keyCatcher.forceActiveFocus()
    if (!hostWidget.openCell(cell)) return
    resetView()
    focusPeriod(destination.lens)
  }
  function currentPeriod() {
    setLens(selectedLens)
    focusPeriod(selectedLens)
  }
  function selectActivity(kind, key) { if (kind) activityType = kind; if (hostWidget) hostWidget.setActivity(kind, key) }
  function inspectInsight(item, control) {
    insightReturnControl = control || null
    inspectedInsight = item
    Qt.callLater(function() { insightBack.forceActiveFocus(); root.revealControl(insightBack) })
  }
  function closeInsight() {
    inspectedInsight = null
    Qt.callLater(function() { if (insightReturnControl) insightReturnControl.forceActiveFocus() })
  }
  function maximum(list) {
    var max = 1
    for (var i = 0; i < list.length; i++) max = Math.max(max, Number(list[i].seconds || list[i].focused_seconds || 0))
    return max
  }
  function revealControl(item) {
    var ancestor = item.parent
    while (ancestor && ancestor !== body) ancestor = ancestor.parent
    if (!ancestor) return
    var y = item.mapToItem(body, 0, 0).y
    if (y < scroll.contentY) scroll.contentY = y
    else if (y + item.height > scroll.contentY + scroll.height) scroll.contentY = Math.max(0, y + item.height - scroll.height)
  }
  function evidenceText(item) { return Model.insightEvidence(item) }

  IpcHandler {
    target: root.moduleName
    function open() { if (root.hostWidget) root.hostWidget.open(); else root.open() }
    function close() { root.close() }
    function show() { if (root.hostWidget) root.hostWidget.open(); else root.open() }
    function hide() { root.close() }
    function toggle() { if (root.hostWidget) root.hostWidget.togglePanel(); else root.toggle() }
    function refresh() { root.refresh() }
    function presentation(): string {
      return JSON.stringify({ opened: root.opened, embedded: !!root.islandContainer,
        width: keyCatcher.width, height: keyCatcher.height,
        sharedSurface: !!root.islandContainer && keyCatcher.parent === root.islandContainer })
    }
    function status(): string { return (root.hostWidget ? root.hostWidget.statusText : root.statusText) || "idle" }
    function period(lens: string, offset: string): void { root.setLensOffset(lens, offset) }
    function day() { root.setLens("day") }
    function week() { root.setLens("week") }
    function month() { root.setLens("month") }
    function year() { root.setLens("year") }
    function life() { root.setLens("life") }
    function activity(kind: string, key: string): void { root.selectActivity(kind, key) }
  }

  DashboardWindow {
    id: panel
    anchorItem: root.anchorItem
    owner: root.hostWidget || root
    bar: root.bar
    open: root.opened && !root.islandContainer
    islandStyle: root.dynamicIslandStyle
    richGraphs: root.richGraphs
    reduceMotion: root.motionDuration === 0
    padding: root.dynamicIslandStyle ? Style.space(32) : Style.spacing.popupPadding
    centerOnBar: true
    margin: Math.max(Style.gapsOut, Style.space(12))
    gap: root.dynamicIslandStyle ? 0 : Math.max(Style.gapsOut, Style.space(8))
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(root.requestedWidth))
    onContentWidthChanged: Qt.callLater(root.syncIsland)
    onContentHeightChanged: Qt.callLater(root.syncIsland)
    contentHeight: panel.fittedContentHeight(Style.space(920), Style.space(920))

    Item {
      id: keyCatcher
      parent: root.islandContainer || panel.contentContainer
      anchors.fill: parent
      focus: true
      Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
          if (root.appearanceOpen) { root.appearanceOpen = false; appearanceButton.forceActiveFocus() }
          else if (root.inspectedInsight) root.closeInsight()
          else if (root.selected) root.selectActivity("", "")
          else root.close()
          event.accepted = true
        } else if (event.key === Qt.Key_Slash) { search.forceActiveFocus(); event.accepted = true }
        else if (event.key === Qt.Key_R) { root.refresh(); event.accepted = true }
      }
      ColumnLayout {
        z: 1
        anchors.fill: parent
        spacing: Style.space(14)
        GridLayout {
          Layout.fillWidth: true
          columns: width >= Style.space(600) ? 2 : 1
          ColumnLayout {
            Layout.fillWidth: true
            spacing: Style.space(3)
            Label { Layout.fillWidth: true; text: root.periodLabel + (root.selectedOffset === 0 && root.selectedLens !== "day" && root.selectedLens !== "life" ? " to date" : ""); font.pixelSize: Style.font.title * 1.2; font.bold: true }
          }
          RowLayout {
            Layout.alignment: Qt.AlignRight
            spacing: Style.space(4)
            Action {
              visible: root.selectedOffset < 0 && root.selectedLens !== "life"
              text: root.selectedLens === "day" ? "Today" : "This " + root.selectedLens
              Accessible.name: "Return to " + text.toLowerCase()
              onClicked: root.currentPeriod()
            }
            Action { text: "‹"; Accessible.name: "Previous period"; enabled: root.selectedLens !== "life"; onClicked: root.setLensOffset(root.selectedLens, root.selectedOffset - 1) }
            Action { text: "›"; Accessible.name: "Next period"; enabled: root.selectedLens !== "life" && root.selectedOffset < 0; onClicked: root.setLensOffset(root.selectedLens, root.selectedOffset + 1) }
            Action {
              id: appearanceButton
              objectName: "appearanceSettingsButton"
              text: "Settings"
              checked: root.appearanceOpen
              Accessible.name: "Dashboard settings"
              onClicked: {
                root.appearanceOpen = !root.appearanceOpen
                if (root.appearanceOpen) Qt.callLater(function() { richGraphsToggle.forceActiveFocus() })
              }
            }
            Action {
              id: refreshAction
              text: root.refreshBusyVisible ? "Refreshing…" : "Refresh"
              implicitWidth: Math.max(Style.space(36), refreshTextMetrics.width + Style.space(22))
              Layout.minimumWidth: implicitWidth
              enabled: !root.refreshRunning
              // Fast requests remain visually quiet while still blocking duplicate clicks.
              opacity: root.refreshBusyVisible ? 0.6 : 1
              Accessible.description: root.refreshRunning ? "Refreshing activity" : ""
              onClicked: root.refresh()
              TextMetrics { id: refreshTextMetrics; font: refreshAction.contentItem.font; text: "Refreshing…" }
            }
          }
        }
        Rectangle {
          objectName: "appearanceSettings"
          Layout.fillWidth: true
          implicitHeight: appearanceBody.implicitHeight + Style.space(24)
          visible: root.appearanceOpen
          color: root.fill
          radius: Style.space(16)
          border.width: 1; border.color: root.line
          ColumnLayout {
            id: appearanceBody
            anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top
            anchors.margins: Style.space(12)
            spacing: Style.space(4)
            RowLayout {
              Layout.fillWidth: true
              Label { text: "Appearance"; font.bold: true; Layout.fillWidth: true }
              Action { text: "Done"; onClicked: { root.appearanceOpen = false; appearanceButton.forceActiveFocus() } }
            }
            Label { text: "Dashboard width"; font.bold: true }
            RowLayout {
              Layout.fillWidth: true
              spacing: Style.space(4)
              Repeater {
                model: [{ label: "Narrow", size: 760 }, { label: "Normal", size: 1160 }, { label: "Wide (4K)", size: 2000 }]
                Action {
                  required property var modelData
                  Layout.fillWidth: true
                  text: modelData.label
                  checked: modelData.size === 760 ? root.requestedWidth < 900
                    : modelData.size === 1160 ? root.requestedWidth >= 900 && root.requestedWidth < 1500
                    : root.requestedWidth >= 1500
                  Accessible.name: modelData.label + " dashboard width"
                  onClicked: root.setAppearance("panelWidth", modelData.size)
                }
              }
            }
            Label {
              Layout.fillWidth: true
              text: "Wide adds a dedicated chart column. Width always fits the available screen."
              color: root.dim
              font.pixelSize: Style.font.caption
            }
            SettingToggle {
              id: richGraphsToggle
              objectName: "richGraphsToggle"
              label: "Rich graphs"; detail: "Soft gradients and curved trends"
              value: root.richGraphs
              onRequested: function(nextValue) { root.setAppearance("richGraphs", nextValue) }
            }
            SettingToggle {
              label: "Reduce motion"; detail: "Show updates without animated transitions"
              value: root.reducedMotion
              onRequested: function(nextValue) { root.setAppearance("reduceMotion", nextValue) }
            }
            SettingToggle {
              label: "Dynamic Island"; detail: "Use the black island presentation"
              value: root.dynamicIslandStyle
              onRequested: function(nextValue) { root.setAppearance("dynamicIslandStyle", nextValue) }
            }
            TrackingStatus {
              Layout.fillWidth: true
              Layout.topMargin: Style.space(8)
              active: root.opened && root.appearanceOpen
              foreground: root.foreground
              dim: root.dim
              fontFamily: root.fontFamily
            }
          }
        }
        Rectangle {
          Layout.fillWidth: true
          id: periodSelector
          implicitHeight: Style.space(44)
          radius: Style.space(root.richGraphs ? 12 : 6)
          color: root.fill
          border.width: 1
          border.color: root.line
          Rectangle {
            visible: root.richGraphs
            x: Style.space(4) + Math.max(0, ["day", "week", "month", "year", "life"].indexOf(root.selectedLens)) * (width + Style.space(4))
            y: Style.space(4)
            width: (parent.width - Style.space(24)) / 5
            height: parent.height - Style.space(8)
            radius: Style.space(9)
            color: root.withAlpha(root.foreground, 0.10)
            border.width: 1; border.color: root.withAlpha(root.foreground, 0.10)
            Behavior on x { enabled: root.opened && root.motionDuration > 0; SmoothedAnimation { velocity: -1; duration: root.motionDuration; maximumEasingTime: 100 } }
          }
          RowLayout {
            anchors.fill: parent
            anchors.margins: Style.space(4)
            spacing: Style.space(4)
            Repeater {
              id: lensButtons
              model: ["day", "week", "month", "year", "life"]
              Action {
                id: lensButton
                required property string modelData
                required property int index
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                Layout.fillHeight: true
                implicitWidth: 0
                implicitHeight: Style.space(36)
                text: modelData === "life" ? "Lifetime" : modelData.charAt(0).toUpperCase() + modelData.slice(1)
                checked: root.selectedLens === modelData
                Accessible.role: Accessible.PageTab
                Accessible.name: text + " view"
                Keys.onLeftPressed: lensButtons.itemAt(Math.max(0, index - 1)).forceActiveFocus()
                Keys.onRightPressed: lensButtons.itemAt(Math.min(4, index + 1)).forceActiveFocus()
                onClicked: root.setLens(modelData)
                contentItem: Label {
                  text: lensButton.text
                  horizontalAlignment: Text.AlignHCenter
                  verticalAlignment: Text.AlignVCenter
                  font.bold: lensButton.checked
                  color: lensButton.checked ? root.accent : root.foreground
                  wrapMode: Text.NoWrap
                }
                background: Rectangle {
                  radius: Style.space(4)
                  color: root.richGraphs ? (lensButton.hovered ? root.fill : "transparent") : (lensButton.checked ? root.withAlpha(root.accent, 0.16) : root.withAlpha(root.foreground, lensButton.hovered ? 0.12 : 0.04))
                  Behavior on color { ColorAnimation { duration: root.motionDuration } }
                  border.width: root.richGraphs ? (lensButton.activeFocus ? 1 : 0) : 1
                  border.color: lensButton.checked || lensButton.activeFocus ? root.accent : root.line
                  Rectangle {
                    visible: lensButton.checked && !root.richGraphs
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: parent.width * 0.45
                    height: Style.space(2)
                    color: root.accent
                  }
                }
              }
            }
          }
        }
            RowLayout {
              Layout.fillWidth: true
              visible: root.errorText !== "" || root.detailError !== ""
              Label { Layout.fillWidth: true; text: (root.errorText || root.detailError) + (root.errorText && root.panelDataLoaded ? " · Showing the last successful report." : ""); color: Color.urgent }
              Action { text: "Retry"; onClicked: root.refresh() }
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
            NumberAnimation { id: dataReveal; target: body; property: "opacity"; from: root.richGraphs ? 0.96 : 0.88; to: 1; duration: root.motionDuration; easing.type: Easing.OutCubic }
            width: scroll.width - Style.space(12)
            spacing: Style.space(10)
            RowLayout {
              Layout.fillWidth: true
              Action { text: "All activity"; visible: root.selected; onClicked: root.selectActivity("", "") }
              Label { Layout.fillWidth: true; text: root.activityLabel; font.pixelSize: Style.font.subtitle; font.bold: true }
              Label { text: root.detailBusyVisible ? "Updating activity…" : ""; color: root.dim }
            }
            GridLayout {
              Layout.fillWidth: true
              columns: root.wide ? (root.selected ? 4 : 3) : 2
              rowSpacing: Style.space(16)
              columnSpacing: Style.space(24)
              Layout.bottomMargin: Style.space(8)
              Metric { primary: true; Layout.columnSpan: root.wide ? 1 : 2; label: "Time spent"; value: root.selected ? root.activityMetrics.time : Model.fmt(root.shownSeconds) }
              Metric { label: root.selected ? "Days used" : "Apps used"; value: root.selected ? root.activityMetrics.days : String((root.activityAnalytics.activities || []).filter(function(a) { return a.kind === "app" }).length) }
              Metric { visible: !root.selected; label: "Multitasked"; value: root.panelDataLoaded ? Model.fmt(root.totalMultitasked) : "—"; detail: root.totalFocused > 0 ? Math.round(root.totalMultitasked / root.totalFocused * 100) + "% of focused time" : "Included in time spent" }
              Metric { visible: root.selected; label: "Visits"; value: root.activityMetrics.visits }
              Metric { visible: root.selected; label: "Typical visit"; value: root.activityMetrics.typical }
            }
            Label {
              Layout.fillWidth: true
              visible: !root.panelDataLoaded || (root.selected && !root.activityDetail) || (root.panelDataLoaded && root.shownSeconds === 0)
              text: !root.panelDataLoaded ? (root.refreshRunning ? "Loading your activity…" : "No report yet. Refresh to try again.")
                : (root.selected && !root.activityDetail ? (root.detailRunning ? "Loading this activity…" : "Activity unavailable. Retry to load this period.") : "No activity recorded in this period.")
              color: root.dim
            }
            GridLayout {
              id: contentGrid
              Layout.fillWidth: true
              columns: root.supportRailVisible ? 3 : root.wide ? 2 : 1
              columnSpacing: Style.space(12)
              rowSpacing: Style.space(12)
              // Each rail sizes its own sections, independently of the chart height.
              GridLayout {
                id: activityRail
                visible: root.wide
                columns: 1
                Layout.row: 0
                Layout.column: 0
                Layout.fillWidth: true
                Layout.preferredWidth: body.width * (root.expansive ? 0.27 : 0.36)
                Layout.minimumWidth: Style.space(290)
                Layout.alignment: Qt.AlignTop
                rowSpacing: Style.space(12)
              }
              GridLayout {
                id: dayActivityRail
                visible: root.wide && root.selectedLens === "day"
                columns: 1
                Layout.row: 0
                Layout.column: 1
                Layout.fillWidth: true
                Layout.preferredWidth: body.width * (root.expansive ? 0.50 : 0.64)
                Layout.alignment: Qt.AlignTop
                rowSpacing: Style.space(12)
              }
              GridLayout {
                id: supportRail
                visible: root.supportRailVisible
                columns: 1
                Layout.row: 0
                Layout.column: 2
                Layout.fillWidth: true
                Layout.preferredWidth: body.width * 0.23
                Layout.alignment: Qt.AlignTop
                rowSpacing: Style.space(16)
              }
              ColumnLayout {
                id: insightsSection
                parent: root.expansive ? supportRail : root.wide ? (root.selectedLens === "day" ? dayActivityRail : activityRail) : contentGrid
                visible: root.hasInsightContent
                Layout.row: root.expansive ? 0 : 2
                Layout.column: 0
                Layout.rowSpan: 1
                Layout.fillWidth: true
                Layout.preferredWidth: root.expansive ? supportRail.width : root.wide ? (root.selectedLens === "day" ? dayActivityRail.width : activityRail.width) : body.width
                Layout.alignment: Qt.AlignTop
                spacing: Style.space(14)
                Label { Layout.fillWidth: true; text: "Patterns & insights"; font.pixelSize: Style.font.subtitle; font.bold: true }
                Label {
                  Layout.fillWidth: true
                  visible: !root.inspectedInsight && root.insights.length === 0
                  text: root.detailRunning || !root.panelDataLoaded ? "Finding the little things in your day…" : "Your story is still taking shape. A few more days of activity will help recurring habits stand out."
                  color: root.dim
                }
                GridLayout {
                  id: insightList
                  visible: !root.inspectedInsight
                  Layout.fillWidth: true
                  columns: 1
                  columnSpacing: Style.space(24)
                  rowSpacing: Style.space(20)
                  readonly property int fittedCount: root.expansive ? 6 : root.wide ? 4 : 2
                  Repeater {
                    id: insightItems
                    model: root.insights
                    Controls.AbstractButton {
                      id: insightButton
                      objectName: "insightEntry" + index
                      required property var modelData
                      required property int index
                      readonly property var presentation: Model.insightPresentation(modelData)
                      Layout.fillWidth: true
                      Layout.minimumWidth: 0
                      Layout.alignment: Qt.AlignTop
                      visible: root.showAllInsights || index < insightList.fittedCount
                      implicitHeight: insightSummary.implicitHeight + Style.space(20)
                      padding: Style.space(10)
                      activeFocusOnTab: true
                      Accessible.name: modelData.title + ". " + presentation.value + ". " + presentation.frequency
                      onActiveFocusChanged: if (activeFocus) root.revealControl(insightButton)
                      onClicked: root.inspectInsight(modelData, insightButton)
                      scale: down ? 0.99 : 1
                      Behavior on scale { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic } }
                      background: Rectangle {
                        radius: Style.space(4)
                        color: insightButton.hovered || insightButton.activeFocus ? root.fill : "transparent"
                        Behavior on color { ColorAnimation { duration: root.motionDuration } }
                        border.width: insightButton.activeFocus ? 1 : 0
                        border.color: root.accent
                        Rectangle { width: parent.width; height: 1; color: root.line }
                      }
                      contentItem: ColumnLayout {
                        id: insightSummary
                        spacing: Style.space(8)
                        RowLayout {
                          Layout.fillWidth: true
                          Label { Layout.fillWidth: true; text: insightButton.modelData.title || "Insight"; wrapMode: Text.Wrap; font.bold: true; font.pixelSize: Style.font.body }
                          Label { text: "›"; color: root.dim; font.pixelSize: Style.font.subtitle }
                        }
                        Label { Layout.fillWidth: true; visible: text.length > 0; text: Model.insightQualifier(insightButton.modelData); color: root.dim; font.pixelSize: Style.font.caption }
                        Label { Layout.fillWidth: true; text: insightButton.presentation.value; wrapMode: Text.Wrap; color: root.accent; font.pixelSize: Style.font.subtitle; font.bold: true }
                        InsightComparison { comparison: (insightButton.modelData.supporting || {}).comparison || null }
                        Label { Layout.fillWidth: true; visible: text.length > 0; text: insightButton.presentation.frequency; color: root.accent }
                        Label { Layout.fillWidth: true; text: Model.insightExplanation(insightButton.modelData); wrapMode: Text.Wrap }
                      }
                    }
                  }
                }
                Action {
                  Layout.fillWidth: true
                  visible: !root.inspectedInsight && root.insights.length > insightList.fittedCount
                  text: root.showAllInsights ? "Show fewer insights" : "Show all " + root.insights.length + " insights · " + (root.insights.length - insightList.fittedCount) + " more"
                  onClicked: root.showAllInsights = !root.showAllInsights
                }
                ColumnLayout {
                  id: insightDetail
                  objectName: "insightDetail"
                  Layout.fillWidth: true
                  visible: root.inspectedInsight !== null
                  readonly property var item: root.inspectedInsight || {}
                  readonly property var presentation: Model.insightPresentation(item)
                  spacing: Style.space(12)
                  opacity: visible ? 1 : 0
                  Behavior on opacity { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic } }
                  Action { id: insightBack; text: "‹ Back to insights"; onClicked: root.closeInsight() }
                  Label { Layout.fillWidth: true; text: insightDetail.item.title || ""; wrapMode: Text.Wrap; font.pixelSize: Style.font.subtitle; font.bold: true }
                  Label { Layout.fillWidth: true; text: insightDetail.presentation.value; wrapMode: Text.Wrap; color: root.accent; font.pixelSize: Style.font.subtitle }
                  Label { Layout.fillWidth: true; text: insightDetail.presentation.frequency; color: root.accent; visible: text.length > 0 }
                  Label { Layout.fillWidth: true; text: Model.insightExplanation(insightDetail.item); wrapMode: Text.Wrap }
                  InsightComparison { comparison: (insightDetail.item.supporting || {}).comparison || null }
                  Label { Layout.fillWidth: true; text: "How we know"; font.bold: true }
                  Label { Layout.fillWidth: true; text: Model.insightQualifier(insightDetail.item); visible: text.length > 0; color: root.dim }
                  Label { Layout.fillWidth: true; text: root.evidenceText(insightDetail.item) || "No additional evidence is available for this finding."; wrapMode: Text.Wrap; color: root.dim }
                  ColumnLayout {
                    Layout.fillWidth: true
                    visible: Model.routineWindows(insightDetail.item).length > 0
                    spacing: Style.space(4)
                    Rectangle {
                      Layout.fillWidth: true
                      implicitHeight: Style.space(7)
                      radius: height / 2
                      color: root.fill
                      Repeater {
                        model: Model.routineWindows(insightDetail.item)
                        Rectangle {
                          required property var modelData
                          x: parent.width * modelData.start
                          width: parent.width * modelData.width
                          height: parent.height
                          radius: height / 2
                          color: root.accent
                        }
                      }
                    }
                    RowLayout {
                      Layout.fillWidth: true
                      Label { text: "Midnight"; color: root.dim; font.pixelSize: Style.font.caption }
                      Item { Layout.fillWidth: true }
                      Label { text: "Noon"; color: root.dim; font.pixelSize: Style.font.caption }
                      Item { Layout.fillWidth: true }
                      Label { text: "Midnight"; color: root.dim; font.pixelSize: Style.font.caption }
                    }
                  }
                  Label { text: "Compared days"; font.bold: true; visible: Model.insightDays(insightDetail.item).length > 0 }
                  Repeater {
                    model: Model.insightDays(insightDetail.item)
                    Label {
                      required property var modelData
                      Layout.fillWidth: true
                      text: Model.insightDate(modelData.date) + " · " + (modelData.matched ? "Pattern appeared" : "Pattern did not appear")
                      wrapMode: Text.Wrap
                      color: root.dim
                      font.pixelSize: Style.font.caption
                    }
                  }
                  Action {
                    Layout.fillWidth: true
                    visible: insightDetail.presentation.activityKey !== "" && insightDetail.presentation.activityKey !== root.selectedActivityKey
                    text: "Explore " + insightDetail.presentation.activityLabel + " →"
                    contentItem: Label { text: parent.text; wrapMode: Text.Wrap; horizontalAlignment: Text.AlignHCenter }
                    onClicked: root.selectActivity(insightDetail.presentation.activityKind, insightDetail.presentation.activityKey)
                  }
                }

              }
              Section {
                parent: root.wide ? activityRail : contentGrid
                title: root.activityType === "app" ? "Time by app" : "Time by website"
                Layout.row: 0
                Layout.column: 0
                Layout.rowSpan: 1
                Layout.fillWidth: true
                Layout.preferredWidth: root.wide ? activityRail.width : body.width
                Layout.alignment: Qt.AlignTop
                GridLayout {
                  Layout.fillWidth: true
                  columns: width >= Style.space(360) ? 2 : 1
                  columnSpacing: Style.space(16)
                  AppDonut {
                    id: compositionDonut
                    objectName: "compositionChart"
                    Layout.fillWidth: true
                    Layout.preferredWidth: Style.space(parent.width < Style.space(440) && parent.columns === 2 ? 120 : 170)
                    Layout.maximumWidth: Layout.preferredWidth
                    Layout.alignment: Qt.AlignHCenter
                    apps: root.composition
                    colors: root.compositionColors
                    totalSeconds: root.composition.reduce(function(n, a) { return n + a.seconds }, 0)
                    highlightedIndex: root.legendHoverIndex >= 0 ? root.legendHoverIndex : root.composition.findIndex(function(a) { return a.kind === root.selectedActivityKind && a.app_class === root.selectedActivityKey })
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
                        onHoveredChanged: { if (hovered) root.legendHoverIndex = index; else if (root.legendHoverIndex === index) root.legendHoverIndex = -1 }
                        opacity: compositionDonut.activeIndex < 0 || compositionDonut.activeIndex === index ? 1 : 0.5
                        Behavior on opacity { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic } }
                        Layout.fillWidth: true
                        implicitHeight: Style.space(28)
                        text: "●  " + modelData.app + " · " + Model.fmt(modelData.seconds) + " · " + modelData.pct + "%"
                        checked: modelData.app_class === root.selectedActivityKey && modelData.kind === root.selectedActivityKind
                        Controls.ToolTip.visible: hovered || activeFocus
                        Controls.ToolTip.text: text
                        onClicked: root.selectActivity(modelData.kind, modelData.app_class)
                        contentItem: RowLayout {
                          spacing: Style.space(8)
                          Rectangle { implicitWidth: Style.space(6); implicitHeight: Style.space(6); radius: width / 2; color: root.colorFromHex(root.compositionColors[index], 1) }
                          Label { Layout.fillWidth: true; Layout.minimumWidth: 0; text: modelData.app; elide: Text.ElideRight; wrapMode: Text.NoWrap; font.pixelSize: Style.font.caption }
                          Label { Layout.preferredWidth: Style.space(78); text: Model.fmt(modelData.seconds); horizontalAlignment: Text.AlignRight; font.pixelSize: Style.font.caption }
                          Label { Layout.preferredWidth: Style.space(42); text: modelData.pct + "%"; horizontalAlignment: Text.AlignRight; font.pixelSize: Style.font.caption }
                        }
                      }
                    }
                    Label { Layout.fillWidth: true; visible: root.composition.length > 5; text: "+" + (root.composition.length - 5) + " more in Explore activity"; color: root.dim; font.pixelSize: Style.font.caption }
                    Label { Layout.fillWidth: true; visible: root.composition.length === 0; text: root.activityType === "domain" ? "No website activity recorded in this period." : "Your time distribution will appear here."; color: root.dim }
                  }
                }
                Label { Layout.fillWidth: true; text: "Overall period · Select an activity to update its charts"; color: root.dim; font.pixelSize: Style.font.caption }
              }
              Section {
                id: rhythmSection
                parent: root.wide && root.selectedLens === "day" ? dayActivityRail : contentGrid
                title: "Your rhythm"
                Layout.row: root.wide ? 0 : 1
                Layout.column: root.wide && root.selectedLens !== "day" ? 1 : 0
                Layout.rowSpan: 1
                Layout.fillWidth: true
                Layout.preferredWidth: root.expansive ? body.width * (root.supportRailVisible ? 0.50 : 0.73) : root.wide ? body.width * 0.64 : body.width
                Layout.alignment: Qt.AlignTop
                GridLayout {
                  id: dayCharts
                  Layout.fillWidth: true
                  visible: root.selectedLens === "day"
                  columns: 1
                  columnSpacing: Style.space(16)
                  rowSpacing: Style.space(12)
                  FocusRing {
                    parent: root.wide ? activityRail : dayCharts
                    visible: root.selectedLens === "day"
                    Layout.row: root.wide ? 1 : 0
                    Layout.column: 0
                    Layout.fillWidth: true
                    Layout.preferredWidth: root.wide ? activityRail.width : dayCharts.width
                    Layout.alignment: Qt.AlignTop
                    Layout.preferredHeight: implicitHeight
                    title: "Busiest hours"
                    detail: "Local time"
                    hours: root.hours
                    maxSeconds: root.maximum(root.hours)
                    expanded: true
                  }
                  DayTimeline {
                    objectName: "dayTimeline"
                    Layout.row: root.wide ? 0 : 1
                    Layout.column: 0
                    Layout.fillWidth: true
                    Layout.preferredWidth: Style.space(600)
                    timeline: root.multitasking.timeline || null
                  }
                }
                FocusTrendLine {
                  objectName: "activityTrend"
                  Layout.fillWidth: true
                  Layout.preferredHeight: implicitHeight
                  visible: root.selectedLens !== "day"
                  title: Model.trendTitle(root.selectedLens)
                  detail: root.selectedLens === "life" ? "Recent 13 weeks" : ""
                  days: root.lineDays
                  maxSeconds: root.maximum(root.lineDays)
                  expanded: true
                  onActivatedCell: function(cell) { root.openCell(cell) }
                }
                Label {
                  Layout.fillWidth: true
                  visible: !root.selected && root.selectedLens !== "day"
                  text: "Multitasked time overlaps focused time · light bars / line show background audio"
                  color: root.dim
                  font.pixelSize: Style.font.caption
                }
                Repeater {
                  model: root.selected || root.selectedLens === "day" ? [] : (root.multitasking.sources || []).slice(0, 5)
                  Label {
                    required property var modelData
                    Layout.fillWidth: true
                    text: modelData.label + " · " + Model.fmt(modelData.seconds) + " alongside other apps"
                    color: root.dim
                    font.pixelSize: Style.font.caption
                  }
                }
                IntensityLegend { Layout.fillWidth: true; visible: root.selectedLens === "month" || root.selectedLens === "year"; contextLabel: root.yearlyRhythm ? "Per week" : "Per calendar day"; maxSeconds: Model.maxHeatSeconds(root.rhythmCells) }
                MonthRhythm {
                  Layout.fillWidth: true
                  Layout.preferredHeight: implicitHeight
                  visible: root.selectedLens === "month" || root.selectedLens === "year"
                  title: root.yearlyRhythm ? "Your year by week" : "Your month at a glance"
                  detail: ""
                  weeklyCells: root.yearlyRhythm
                  cells: root.rhythmCells
                  weeks: root.rhythmWeeks
                  weekdays: root.calendarWeekdays
                  maxSeconds: root.maximum(root.rhythmCells)
                  weekMaxSeconds: root.maximum(root.rhythmWeeks)
                  weekdayMaxSeconds: root.maximum(root.calendarWeekdays)
                  onActivatedCell: function(cell) { root.openCell(cell) }
                }
                Label { Layout.fillWidth: true; Layout.minimumHeight: Style.space(32); visible: root.selectedLens !== "day" && root.chartReadout.length > 0; text: root.chartReadout || "Point to an hour to inspect focused and multitasked time."; color: root.dim; font.pixelSize: Style.font.caption }
                ColumnLayout {
                  // The wide view pairs the calendar with hourly patterns in its own rail.
                  parent: root.expansive && root.calendarLens ? supportRail : rhythmSection.contentLayout
                  Layout.row: root.expansive && root.calendarLens ? 1 : -1
                  Layout.column: 0
                  Layout.fillWidth: true
                  Layout.alignment: Qt.AlignTop
                  visible: root.selectedLens !== "day"
                  spacing: Style.space(8)
                  Label { text: "Time spent by day and hour"; font.bold: true }
                  IntensityLegend { Layout.fillWidth: true; maxSeconds: Model.maxHeatSeconds(root.heatCells) }
                  Heatmap { Layout.fillWidth: true }
                }

              }
              Section {
                parent: root.wide ? (root.selectedLens === "day" ? dayActivityRail : activityRail) : contentGrid
                title: "Explore activity"
                Layout.row: root.wide ? 1 : 3
                Layout.column: 0
                Layout.rowSpan: 1
                Layout.fillWidth: true
                Layout.preferredWidth: root.wide ? (root.selectedLens === "day" ? dayActivityRail.width : activityRail.width) : body.width
                Layout.alignment: Qt.AlignTop
                RowLayout {
                  Layout.fillWidth: true
                  Item { Layout.fillWidth: true }
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
                  onActiveFocusChanged: if (activeFocus) Qt.callLater(function() { root.revealControl(search) })
                  selectByMouse: true
                  padding: Style.space(10)
                  background: Rectangle { color: root.fill; border.width: 1; border.color: search.activeFocus ? root.accent : root.line; radius: Style.space(4) }
                  onTextChanged: root.showAllActivities = false
                }
                Label {
                  Layout.fillWidth: true
                  visible: root.activityType === "domain"
                  text: root.activityAnalytics.browser_domains_enabled === false ? "Website tracking is disabled in your privacy settings."
                    : "Website time is part of browser time."
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
                      onActiveFocusChanged: if (activeFocus) Qt.callLater(function() { root.revealControl(this) }.bind(this))
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
                          Label { Layout.fillWidth: true; text: Model.visitSummary(activityButton.modelData); color: root.dim; font.pixelSize: Style.font.caption }
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
                Label { Layout.fillWidth: true; visible: root.filteredActivities.length === 0; text: search.text ? "No matching activity." : (root.activityType === "domain" ? "No websites recorded in this period. Enable the browser extension to see them here." : "No apps recorded in this period."); color: root.dim }
                Action { text: root.showAllActivities ? "Show fewer" : "Show all " + root.filteredActivities.length; visible: root.filteredActivities.length > 8 && !search.text.length; onClicked: root.showAllActivities = !root.showAllActivities }
              }
            }
            Action { text: root.dataExpanded ? "About your data ↑" : "About your data ↓"; checked: root.dataExpanded; onClicked: root.dataExpanded = !root.dataExpanded }
            ColumnLayout {
              Layout.fillWidth: true
              visible: root.dataExpanded
              spacing: Style.space(10)
              Label { Layout.fillWidth: true; text: "Time spent counts the app you were actually using. Time away is left out. Returning within five minutes counts as the same visit; a typical visit is the middle visit length when sorted from shortest to longest."; color: root.dim }
              Label { Layout.fillWidth: true; text: "Time away · " + Model.fmt(root.totalIdle) + " idle · " + Model.fmt(root.totalLocked) + " locked · " + Model.fmt(root.totalSleep) + " asleep"; color: root.dim }
              Label { Layout.fillWidth: true; text: "Tracking gaps · " + Model.fmt(root.totalUnobserved) + ". These are left out when looking for habits."; color: root.dim }
              Label { Layout.fillWidth: true; text: "Website time is part of browser time. " + Model.fmt(root.activityAnalytics.unattributed_browser_seconds || 0) + " of browser use has no website recorded."; color: root.dim }
              Label { Layout.fillWidth: true; text: "Habit history: " + root.baselineText; color: root.dim }
              Label { text: root.updatedText ? "Last updated " + root.updatedText : "Waiting for a report"; color: root.dim }
            }
          }
        }
      }
    }
  }

  component Section: Rectangle {
    id: section
    property string title: ""
    default property alias contents: sectionBody.data
    readonly property alias contentLayout: sectionBody
    readonly property int inset: Style.space(root.richGraphs ? 16 : 10)
    implicitHeight: sectionBody.implicitHeight + inset * 2
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, root.dynamicIslandStyle ? 0.045 : 0.025)
    border.width: root.richGraphs ? 0 : 1
    border.color: root.line
    radius: Style.space(root.richGraphs ? 18 : (root.dynamicIslandStyle ? 12 : 6))
    gradient: root.richGraphs ? sectionGradient : null
    Gradient {
      id: sectionGradient
      GradientStop { position: 0; color: root.withAlpha(root.foreground, 0.065) }
      GradientStop { position: 1; color: root.withAlpha(root.foreground, 0.025) }
    }
    ColumnLayout {
      id: sectionBody
      x: section.inset
      y: section.inset
      width: parent.width - section.inset * 2
      spacing: Style.space(8)
      Label { Layout.fillWidth: true; text: section.title; font.pixelSize: Style.font.subtitle; font.bold: true }
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
    implicitHeight: Math.max(Style.space(36), contentItem.implicitHeight + Style.space(16))
    activeFocusOnTab: true
    onActiveFocusChanged: if (activeFocus) Qt.callLater(function() { root.revealControl(this) }.bind(this))
    Accessible.name: text
    opacity: enabled ? 1 : 0.4
    scale: down ? 0.98 : 1
    Behavior on scale { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic } }
    contentItem: Label { text: action.text; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter; font.bold: action.checked }
    background: Rectangle { radius: Style.space(4); color: action.checked || action.hovered ? root.fill : "transparent"; Behavior on color { ColorAnimation { duration: root.motionDuration } } border.width: action.checked || action.activeFocus ? 1 : 0; border.color: action.activeFocus ? root.accent : root.line }
  }
  component SettingToggle: Controls.AbstractButton {
    id: preference
    property string label: ""
    property string detail: ""
    property bool value: false
    signal requested(bool nextValue)
    Layout.fillWidth: true
    implicitHeight: Math.max(Style.space(50), preferenceContent.implicitHeight + Style.space(12))
    activeFocusOnTab: true
    Accessible.role: Accessible.CheckBox
    Accessible.name: label
    Accessible.description: detail
    Accessible.checked: value
    onClicked: requested(!value)
    background: Rectangle {
      radius: Style.space(10)
      color: preference.hovered ? root.fill : "transparent"
      border.width: preference.activeFocus ? 1 : 0; border.color: root.accent
    }
    contentItem: RowLayout {
      id: preferenceContent
      spacing: Style.space(12)
      ColumnLayout {
        Layout.fillWidth: true; spacing: Style.space(2)
        Label { Layout.fillWidth: true; text: preference.label; font.bold: true }
        Label { Layout.fillWidth: true; text: preference.detail; color: root.dim; font.pixelSize: Style.font.caption }
      }
      Rectangle {
        Layout.preferredWidth: Style.space(42); Layout.preferredHeight: Style.space(24)
        radius: height / 2
        color: preference.value ? root.accent : root.withAlpha(root.foreground, 0.18)
        Behavior on color { ColorAnimation { duration: root.motionDuration } }
        Rectangle {
          x: preference.value ? parent.width - width - Style.space(2) : Style.space(2)
          y: Style.space(2); width: Style.space(20); height: width; radius: width / 2
          color: "#ffffff"
          Behavior on x { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutQuint } }
        }
      }
    }
  }
  component Metric: ColumnLayout {
    property string label: ""
    property string value: ""
    property string detail: ""
    property bool primary: false
    Layout.fillWidth: true
    Layout.preferredWidth: primary ? 1.6 : 1
    Layout.alignment: Qt.AlignTop
    spacing: Style.space(4)
    Label { Layout.fillWidth: true; text: parent.label; color: root.dim; font.pixelSize: Style.font.caption }
    Label { Layout.fillWidth: true; text: parent.value; font.pixelSize: Style.font.title * (parent.primary ? (root.richGraphs ? 1.85 : 1.5) : 1.05); font.bold: parent.primary; color: parent.primary ? root.foreground : root.dim }
    Label { visible: parent.detail.length > 0; Layout.fillWidth: true; text: parent.detail; color: root.dim; font.pixelSize: Style.font.caption }
  }
  component InsightComparison: ColumnLayout {
    id: comparisonChart
    property var comparison: null
    readonly property real maximum: comparison ? Math.max(1, comparison.baseline_seconds, comparison.current_seconds) : 1
    Layout.fillWidth: true
    visible: comparison !== null
    spacing: Style.space(6)
    Repeater {
      model: comparisonChart.comparison ? [
        { label: "Earlier weekdays", seconds: comparisonChart.comparison.baseline_seconds, current: false },
        { label: "Recent weekdays", seconds: comparisonChart.comparison.current_seconds, current: true }
      ] : []
      RowLayout {
        required property var modelData
        Layout.fillWidth: true
        Label { Layout.preferredWidth: Style.space(104); text: modelData.label; color: root.dim; font.pixelSize: Style.font.caption }
        Rectangle {
          Layout.fillWidth: true
          implicitHeight: Style.space(6)
          color: root.fill
          radius: height / 2
          Rectangle { width: parent.width * root.clamp01(modelData.seconds / comparisonChart.maximum); height: parent.height; radius: height / 2; color: modelData.current ? root.accent : root.withAlpha(root.foreground, 0.4) }
        }
        Label { Layout.preferredWidth: Style.space(54); text: Model.fmt(modelData.seconds); horizontalAlignment: Text.AlignRight; font.pixelSize: Style.font.caption }
      }
    }
  }

  component DayTimeline: ColumnLayout {
    id: dayMapRoot
    property var timeline: null
    property bool fitSpan: true
    property int selectedIndex: -1
    property real hoveredTime: -1
    property real pinnedTime: -1
    readonly property var chartData: Model.dayMap(timeline, fitSpan, 6)
    readonly property var colors: Model.stableAppColors(chartData.lanes, String(root.accent))
    readonly property real labelWidth: Style.space(width < Style.space(500) ? 104 : 136)
    readonly property real rowHeight: Style.space(38)
    readonly property real inspectedTime: hoveredTime >= 0 ? hoveredTime : pinnedTime >= 0 ? pinnedTime : selectedIndex >= 0 && selectedIndex < chartData.segments.length ? chartData.segments[selectedIndex].start : -1
    readonly property int inspectedIndex: inspectedTime >= 0 ? Model.timelineIndexAt(chartData.segments, inspectedTime) : -1
    readonly property var inspected: inspectedIndex >= 0 ? chartData.segments[inspectedIndex] : null
    readonly property real audioSeconds: chartData.audio.reduce(function(n, span) { return n + span.end - span.start }, 0)
    readonly property string readout: inspected
      ? Model.timelineClock(inspected.start) + "–" + Model.timelineClock(inspected.end) + " · " + inspected.label + " · " + Model.fmtPrecise(inspected.end - inspected.start) + "\n" + Model.timelineAudioLabel(inspected)
      : inspectedTime >= 0 ? Model.timelineClock(inspectedTime) + (inspectedTime >= chartData.observedEnd ? " · Outside the recorded period" : " · No focused activity recorded")
      : chartData.segments.length ? "Point to a moment · Click to pin · Arrow keys step through activity · Enter opens the app" : !root.panelDataLoaded ? "Loading your activity timeline…" : !timeline ? "Activity timeline unavailable for this report." : "No focused activity recorded in this day."
    spacing: Style.space(12)
    onTimelineChanged: { selectedIndex = -1; hoveredTime = -1; pinnedTime = -1 }
    Connections { target: root; function onViewKeyChanged() { dayMapRoot.selectedIndex = -1; dayMapRoot.hoveredTime = -1; dayMapRoot.pinnedTime = -1; dayMapRoot.fitSpan = true } }
    function position(timestamp) { return (timestamp - chartData.start) / Math.max(1, chartData.end - chartData.start) }
    function step(direction) {
      hoveredTime = -1; pinnedTime = -1
      selectedIndex = Math.max(0, Math.min(chartData.segments.length - 1, selectedIndex + direction))
    }
    function activate() { if (inspected) root.selectActivity("app", inspected.app_class) }

    RowLayout {
      Layout.fillWidth: true
      Label { text: "Your day, in sequence"; font.bold: true; Layout.fillWidth: true }
      Action { text: dayMapRoot.fitSpan ? "Active span" : "Full day"; enabled: dayMapRoot.chartData.segments.length > 0; checked: dayMapRoot.fitSpan; Accessible.name: "Timeline range: " + text + ". Activate to switch range"; onClicked: dayMapRoot.fitSpan = !dayMapRoot.fitSpan }
    }
    Label {
      Layout.fillWidth: true
      text: "Overall app timeline · Local time" + (root.selected ? " · Clock shows " + root.activityLabel : "")
      color: root.dim
      font.pixelSize: Style.font.caption
    }
    RowLayout {
      Layout.fillWidth: true
      visible: dayMapRoot.chartData.segments.length > 0
      spacing: Style.space(16)
      Metric { label: "Longest stretch"; value: dayMapRoot.chartData.longest ? Model.fmt(dayMapRoot.chartData.longest.end - dayMapRoot.chartData.longest.start) : "—"; detail: dayMapRoot.chartData.longest ? dayMapRoot.chartData.longest.label : "" }
      Metric { label: "App switches"; value: String(dayMapRoot.chartData.switches); detail: "During continuous activity" }
      Metric { label: "Audio overlap"; value: Model.fmt(dayMapRoot.audioSeconds); detail: "Included in focused time" }
    }
    Item {
      id: dayPlot
      objectName: "dayTimelinePlot"
      Layout.fillWidth: true
      implicitHeight: (dayMapRoot.chartData.lanes.length + 1) * dayMapRoot.rowHeight + Style.space(28)
      visible: dayMapRoot.chartData.segments.length > 0
      activeFocusOnTab: visible
      Accessible.role: Accessible.Button
      Accessible.name: "Day timeline. " + dayMapRoot.readout
      onActiveFocusChanged: if (activeFocus) { if (dayMapRoot.selectedIndex < 0) dayMapRoot.selectedIndex = 0; Qt.callLater(function() { root.revealControl(dayReadout) }) }
      Keys.onLeftPressed: dayMapRoot.step(-1)
      Keys.onRightPressed: dayMapRoot.step(1)
      Keys.onReturnPressed: dayMapRoot.activate()
      Keys.onSpacePressed: dayMapRoot.activate()
      Repeater {
        model: dayMapRoot.chartData.lanes
        Action {
          required property var modelData
          required property int index
          x: 0; y: index * dayMapRoot.rowHeight
          width: dayMapRoot.labelWidth - Style.space(12)
          height: dayMapRoot.rowHeight
          text: modelData.label
          Accessible.name: modelData.label + ", " + Model.fmtPrecise(modelData.seconds) + (modelData.app_class ? ". Explore app" : "")
          enabled: modelData.app_class.length > 0
          opacity: 1
          onClicked: root.selectActivity("app", modelData.app_class)
          Controls.ToolTip.visible: hovered || activeFocus
          Controls.ToolTip.text: Accessible.name
          contentItem: Column {
            spacing: Style.space(2)
            Label { width: parent.width; text: modelData.label; elide: Text.ElideRight; wrapMode: Text.NoWrap; font.pixelSize: Style.font.caption; color: root.colorFromHex(dayMapRoot.colors[index], 1) }
            Label { width: parent.width; text: Model.fmt(modelData.seconds); color: root.dim; font.pixelSize: Style.font.caption }
          }
        }
      }
      Label { x: 0; y: dayMapRoot.chartData.lanes.length * dayMapRoot.rowHeight + Style.space(10); width: dayMapRoot.labelWidth; text: "Background audio"; color: root.dim; font.pixelSize: Style.font.caption }
      Item {
        id: dayTracks
        x: dayMapRoot.labelWidth
        width: Math.max(1, parent.width - x)
        height: (dayMapRoot.chartData.lanes.length + 1) * dayMapRoot.rowHeight
        clip: true
        Canvas {
          id: dayCanvas
          anchors.fill: parent
          onWidthChanged: requestPaint()
          onHeightChanged: requestPaint()
          Connections { target: dayMapRoot; function onChartDataChanged() { dayCanvas.requestPaint() } function onColorsChanged() { dayCanvas.requestPaint() } }
          Connections { target: root; function onForegroundChanged() { dayCanvas.requestPaint() } }
          onPaint: {
            var ctx = getContext("2d")
            ctx.reset()
            var chartData = dayMapRoot.chartData, row = dayMapRoot.rowHeight
            // Neutral rows keep time away visible without inventing an idle reason.
            ctx.fillStyle = root.canvasColor(root.foreground, 0.045)
            for (var r = 0; r <= chartData.lanes.length; r++) ctx.fillRect(0, r * row + Style.space(8), width, Style.space(22))
            ctx.fillStyle = root.canvasColor(root.foreground, 0.08)
            for (var t = 0; t <= 4; t++) ctx.fillRect(Math.round(width * t / 4), 0, 1, height)
            function spanRect(span, y, h) {
              var x = Math.max(0, dayMapRoot.position(span.start) * width)
              var end = Math.min(width, dayMapRoot.position(span.end) * width)
              if (end > x) ctx.fillRect(x, y, Math.max(1, end - x), h)
            }
            for (var i = 0; i < chartData.lanes.length; i++) {
              ctx.fillStyle = dayMapRoot.colors[i]
              for (var j = 0; j < chartData.lanes[i].spans.length; j++) spanRect(chartData.lanes[i].spans[j], i * row + Style.space(8), Style.space(22))
            }
            ctx.fillStyle = root.canvasColor(root.foreground, 0.7)
            for (var k = 0; k < chartData.audio.length; k++) spanRect(chartData.audio[k], chartData.lanes.length * row + Style.space(14), Style.space(10))
          }
        }
        Rectangle {
          visible: dayMapRoot.inspectedTime >= dayMapRoot.chartData.start && dayMapRoot.inspectedTime <= dayMapRoot.chartData.end
          x: Math.min(parent.width - width, Math.max(0, dayMapRoot.position(dayMapRoot.inspectedTime) * parent.width))
          height: parent.height; width: Style.space(1)
          color: root.foreground
        }
        Rectangle { anchors.fill: parent; color: "transparent"; border.width: dayPlot.activeFocus ? 1 : 0; border.color: root.accent }
        MouseArea {
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onPositionChanged: function(mouse) { dayMapRoot.hoveredTime = dayMapRoot.chartData.start + Math.min(0.999999, Math.max(0, mouse.x / width)) * (dayMapRoot.chartData.end - dayMapRoot.chartData.start) }
          onExited: dayMapRoot.hoveredTime = -1
          onClicked: function(mouse) {
            var time = dayMapRoot.chartData.start + Math.min(0.999999, Math.max(0, mouse.x / width)) * (dayMapRoot.chartData.end - dayMapRoot.chartData.start)
            dayPlot.forceActiveFocus()
            dayMapRoot.selectedIndex = Model.timelineIndexAt(dayMapRoot.chartData.segments, time)
            dayMapRoot.pinnedTime = time
          }
          onDoubleClicked: dayMapRoot.activate()
        }
      }
      Repeater {
        model: dayMapRoot.width < Style.space(500) ? 3 : 5
        Label {
          required property int index
          readonly property int count: dayMapRoot.width < Style.space(500) ? 3 : 5
          readonly property real fraction: index / (count - 1)
          width: Style.space(58)
          x: dayMapRoot.labelWidth + Math.max(0, Math.min(dayTracks.width - width, dayTracks.width * fraction - width / 2))
          y: dayTracks.height + Style.space(4)
          text: Model.timelineClock(dayMapRoot.chartData.start + fraction * (dayMapRoot.chartData.end - dayMapRoot.chartData.start))
          color: root.dim
          font.pixelSize: Style.font.caption
          horizontalAlignment: index === 0 ? Text.AlignLeft : index === count - 1 ? Text.AlignRight : Text.AlignHCenter
        }
      }
    }
    ChartReadout { id: dayReadout; Layout.fillWidth: true; text: dayMapRoot.readout; reservedLines: 2 }
    Label { Layout.fillWidth: true; visible: dayMapRoot.chartData.segments.length > 0; text: "Blank space means no focused activity was recorded. Audio overlaps focused time."; color: root.dim; font.pixelSize: Style.font.caption }
    Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: root.line; visible: audioSources.count > 0 }
    Label { text: "Alongside your day"; font.bold: true; visible: audioSources.count > 0 }
    GridLayout {
      Layout.fillWidth: true
      columns: width >= Style.space(600) ? 2 : 1
      columnSpacing: Style.space(20)
      rowSpacing: Style.space(4)
      Repeater {
        id: audioSources
        model: (root.multitasking.sources || []).slice(0, 6)
        Action {
          required property var modelData
          Layout.fillWidth: true
          Layout.minimumWidth: 0
          implicitHeight: Style.space(46)
          Accessible.name: modelData.label + ", " + Model.fmtPrecise(modelData.seconds) + " alongside other apps. Explore activity"
          onClicked: root.selectActivity(modelData.attribution === "domain" ? "domain" : "app", modelData.attribution === "domain" ? modelData.label : modelData.app_class)
          contentItem: ColumnLayout {
            spacing: Style.space(6)
            RowLayout {
              Layout.fillWidth: true
              Label { Layout.fillWidth: true; Layout.minimumWidth: 0; text: modelData.label; elide: Text.ElideRight; wrapMode: Text.NoWrap }
              Label { text: Model.fmt(modelData.seconds); color: root.dim; font.pixelSize: Style.font.caption }
            }
            Rectangle { Layout.fillWidth: true; implicitHeight: Style.space(3); color: root.fill
              Rectangle { width: parent.width * root.clamp01(Number(modelData.seconds) / Math.max(1, root.totalMultitasked)); height: parent.height; color: root.withAlpha(root.foreground, 0.6); radius: height / 2 }
            }
          }
        }
      }
    }
  }

  component IntensityLegend: RowLayout {
    property real maxSeconds: 0
    property string contextLabel: "Time spent"
    readonly property var labels: Model.durationLegend(maxSeconds)
    spacing: Style.space(5)
    Label { text: parent.labels[0]; color: root.dim; font.pixelSize: Style.font.caption }
    Repeater {
      model: 5
      Rectangle {
        required property int index
        implicitWidth: Style.space(15)
        implicitHeight: Style.space(10)
        color: index === 0 ? root.fill : root.accent
        opacity: index === 0 ? 1 : 0.2 + index * 0.2
      }
    }
    Label { text: parent.labels[1]; color: root.dim; font.pixelSize: Style.font.caption }
    Item { Layout.fillWidth: true }
    Label { text: parent.contextLabel; color: root.dim; font.pixelSize: Style.font.caption }
  }
  component Heatmap: ColumnLayout {
    id: heatmapRoot
    property string selectedText: ""
    Connections { target: root; function onViewKeyChanged() { heatmapRoot.selectedText = "" } }
    function focusCell(day, hour) {
      var row = heatRows.itemAt(Math.max(0, Math.min(6, day)))
      if (row) row.buttons.itemAt(Math.max(0, Math.min(23, hour))).forceActiveFocus()
    }
    spacing: Style.space(4)
    RowLayout {
      Layout.fillWidth: true
      Label { text: ""; Layout.preferredWidth: Style.space(32) }
      Repeater { model: ["00:00", "06:00", "12:00", "18:00"]; Label { required property string modelData; Layout.fillWidth: true; text: modelData; color: root.dim; font.pixelSize: Style.font.caption } }
    }
    Repeater {
      id: heatRows
      model: 7
      RowLayout {
        id: heatRow
        required property int index
        property int weekday: index
        readonly property var buttons: heatButtons
        Layout.fillWidth: true
        spacing: Style.space(3)
        Label { text: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][parent.weekday]; Layout.preferredWidth: Style.space(32); font.pixelSize: Style.font.caption; color: root.dim }
        Repeater {
          id: heatButtons
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
            onActiveFocusChanged: if (activeFocus) { heatmapRoot.selectedText = Accessible.name; Qt.callLater(function() { root.revealControl(this) }.bind(this)) }
            Keys.onLeftPressed: heatmapRoot.focusCell(weekday, index - 1)
            Keys.onRightPressed: heatmapRoot.focusCell(weekday, index + 1)
            Keys.onUpPressed: heatmapRoot.focusCell(weekday - 1, index)
            Keys.onDownPressed: heatmapRoot.focusCell(weekday + 1, index)
            Keys.onPressed: function(event) {
              if (event.key === Qt.Key_Home || event.key === Qt.Key_End) {
                heatmapRoot.focusCell(weekday, event.key === Qt.Key_Home ? 0 : 23)
                event.accepted = true
              }
            }
            Accessible.name: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"][weekday] + " " + index + ":00 · " + Model.fmtPrecise(seconds)
            onClicked: heatmapRoot.selectedText = Accessible.name
            background: Rectangle { radius: Style.space(2); color: cell.seconds > 0 ? root.accent : root.fill; opacity: cell.seconds > 0 ? 0.2 + 0.8 * cell.seconds / root.maximum(root.heatCells) : 1; border.width: cell.activeFocus || cell.hovered ? 2 : 0; border.color: root.foreground }
            Controls.ToolTip.visible: hovered || activeFocus
            Controls.ToolTip.text: Accessible.name
          }
        }
      }
    }
    Label {
      Layout.fillWidth: true
      Layout.minimumHeight: Style.space(26)
      text: heatmapRoot.selectedText || "Select an hour to inspect · Arrow keys move through the week"
      color: root.dim
      font.pixelSize: Style.font.caption
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
    readonly property string centerLabel: hasActiveApp ? root.formatDuration(Number((apps[activeIndex] || {}).seconds || 0)) + " · " + Math.round(Number((apps[activeIndex] || {}).seconds || 0) / Math.max(1, totalSeconds) * 100) + "%" : root.formatDuration(totalSeconds)
    readonly property string centerDetail: hasActiveApp ? String((apps[activeIndex] || {}).app || "App") : "Time spent"
    Controls.ToolTip.visible: hoveredIndex >= 0
    Controls.ToolTip.text: centerDetail + " · " + Model.fmtPrecise(hasActiveApp ? apps[activeIndex].seconds : totalSeconds)
    readonly property int chartSize: Math.min(Style.space(150), width - Style.space(8))

    Layout.minimumHeight: Style.space(158)
    implicitHeight: Style.space(158)
    scale: hoveredIndex >= 0 ? 1.015 : 1
    Behavior on scale { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic } }

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
      anchors.margins: Style.space(4)
      spacing: Style.space(8)

      Item {
        width: parent.width
        height: donutRoot.chartSize

        Canvas {
          id: donutCanvas

          // Canvas pixels do not follow QML color bindings until repainted.
          Connections {
            target: root
            function onAccentChanged() { donutCanvas.requestPaint() }
            function onForegroundChanged() { donutCanvas.requestPaint() }
          function onRichGraphsChanged() { donutCanvas.requestPaint() }
          }

          anchors.centerIn: parent
          width: donutRoot.chartSize
          height: donutRoot.chartSize
          antialiasing: true
        opacity: root.richGraphs ? 0.72 + 0.28 * root.graphProgress : 1
        transform: Scale {
          origin.x: donutCanvas.width / 2; origin.y: donutCanvas.height
          xScale: 1
          yScale: root.richGraphs ? 0.96 + 0.04 * root.graphProgress : 1
        }

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
              if (root.richGraphs) {
                var sheen = ctx.createLinearGradient(0, 0, width, height)
                sheen.addColorStop(0, root.canvasColor(Qt.lighter(color, 1.15), color.a))
                sheen.addColorStop(1, root.canvasColor(color, color.a * 0.78))
                ctx.strokeStyle = sheen
              } else ctx.strokeStyle = root.canvasColor(color, color.a)
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
    readonly property int chartSize: expanded ? Math.max(Style.space(146), Math.min(Style.space(root.expansive ? 260 : 210), width - Style.space(128))) : Style.space(132)
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < hours.length
      ? hourlyDetailText(hours[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property var peak: Model.bestHour(hours)
    readonly property string defaultText: peak.label !== "--" ? "Peak " + peak.label + ": " + peak.value + "  " + peak.detail : ""

    function hourlyDetailText(cell) {
      if (!cell) return ""
      return String(cell.fullLabel || cell.label || "Hour") + ": " + Model.fmtPrecise(Number(cell.seconds || 0)) + " focused" + (Number(cell.multitasked_seconds || 0) > 0 ? " · " + Model.fmtPrecise(cell.multitasked_seconds) + " multitasked" : "")
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

    implicitHeight: chartSize + Style.space(118)
    radius: 0
    color: root.noFill
    border.width: 0

    activeFocusOnTab: visible
    onActiveFocusChanged: if (activeFocus) { selectedIndex = Math.max(0, selectedIndex); root.revealControl(ringRoot) }
    Keys.onLeftPressed: selectedIndex = Math.max(0, selectedIndex - 1)
    Keys.onRightPressed: selectedIndex = Math.min(hours.length - 1, selectedIndex + 1)
    onHoursChanged: { selectedIndex = -1; hoveredIndex = -1; hoveredText = ""; ringCanvas.requestPaint() }
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

        // Canvas pixels do not follow QML color bindings until repainted.
        Connections {
          target: root
          function onAccentChanged() { ringCanvas.requestPaint() }
          function onForegroundChanged() { ringCanvas.requestPaint() }
          function onRichGraphsChanged() { ringCanvas.requestPaint() }
        }

        width: ringRoot.chartSize
        height: ringRoot.chartSize
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: parent.verticalCenter
        antialiasing: true
        opacity: root.richGraphs ? 0.72 + 0.28 * root.graphProgress : 1
        transform: Scale {
          origin.x: ringCanvas.width / 2; origin.y: ringCanvas.height
          xScale: 1
          yScale: root.richGraphs ? 0.96 + 0.04 * root.graphProgress : 1
        }

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
            ctx.strokeStyle = root.canvasColor(root.accent, active ? 1.0 : 0.36 + intensity * 0.54)
            ctx.lineWidth = active ? activeWidth + Style.space(3) : activeWidth
            ctx.stroke()
          }
        }
      }

      Label { anchors.horizontalCenter: ringCanvas.horizontalCenter; anchors.bottom: ringCanvas.top; text: "Midnight"; color: root.dim; font.pixelSize: Style.font.caption }
      Label { anchors.horizontalCenter: ringCanvas.horizontalCenter; anchors.top: ringCanvas.bottom; text: "Noon"; color: root.dim; font.pixelSize: Style.font.caption }
      Label { anchors.right: ringCanvas.left; anchors.verticalCenter: ringCanvas.verticalCenter; text: "Evening"; color: root.dim; font.pixelSize: Style.font.caption }
      Label { anchors.left: ringCanvas.right; anchors.verticalCenter: ringCanvas.verticalCenter; text: "Morning"; color: root.dim; font.pixelSize: Style.font.caption }
      Column {
        anchors.centerIn: ringCanvas
        width: ringCanvas.width * 0.64
        Label { width: parent.width; text: ringRoot.peak.label; horizontalAlignment: Text.AlignHCenter; font.bold: true }
        Label { width: parent.width; text: ringRoot.peak.value; horizontalAlignment: Text.AlignHCenter; color: root.dim; font.pixelSize: Style.font.caption }
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
      text: ringRoot.readoutText || "Brighter segments mean more time spent during that hour."
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
    readonly property string readoutText: inspectedDay ? Model.trendDetailText(inspectedDay) + (isCurrentDay(inspectedDay) ? " so far" : "") : ""
    readonly property bool hasActivity: days.some(function(day) { return Number(day.seconds || 0) > 0 })
    readonly property string defaultText: Model.trendDefaultText(days, root.selectedLens === "year" || root.selectedLens === "life" ? "week" : "day")
    readonly property int inspectedIndex: hoveredIndex >= 0 ? hoveredIndex : selectedIndex
    readonly property var inspectedDay: inspectedIndex >= 0 && inspectedIndex < days.length ? days[inspectedIndex] : null
    function isCurrentDay(cell) { return cell && !cell.monthly && !cell.weekly && String(cell.date || cell.key || "") === root.todayKey }
    readonly property real averageSeconds: {
      var list = days || []
      if (list.length <= 0) return 0
      var total = 0
      for (var i = 0; i < list.length; i++) total += Number(list[i].seconds || 0)
      return total / list.length
    }

    readonly property real plotInset: Style.space(6)
    readonly property var focusCurve: Model.smoothChartSegments(days, "seconds")
    readonly property var mediaCurve: Model.smoothChartSegments(days, "multitasked_seconds", focusCurve)
    function traceCurve(ctx, field, connect) {
      if (!days.length) return
      var firstY = pointY(Number(days[0][field] || 0))
      if (connect) ctx.lineTo(pointX(0), firstY); else ctx.moveTo(pointX(0), firstY)
      var curve = field === "seconds" ? focusCurve : mediaCurve
      for (var i = 0; i < days.length - 1; i++) {
        var x = pointX(i), nextX = pointX(i + 1), dx = (nextX - x) / 3
        if (root.richGraphs) ctx.bezierCurveTo(x + dx, pointY(curve[i].c1), nextX - dx, pointY(curve[i].c2), nextX, pointY(curve[i].to))
        else ctx.lineTo(nextX, pointY(Number(days[i + 1][field] || 0)))
      }
    }
    function pointX(index) {
      var count = Math.max(1, days.length)
      if (count === 1) return linePlot.width / 2
      return plotInset + index * Math.max(0, linePlot.width - 2 * plotInset) / (count - 1)
    }

    function pointY(seconds) {
      var value = root.clamp01(Number(seconds || 0) / Math.max(1, lineRoot.maxSeconds))
      return plotInset + (1 - value) * Math.max(0, linePlot.height - 2 * plotInset)
    }

    function indexAt(x) {
      var count = days.length
      if (count <= 0 || linePlot.width <= 0) return -1
      if (count === 1) return 0
      return Math.max(0, Math.min(count - 1, Math.round((x - plotInset) / Math.max(1, linePlot.width - 2 * plotInset) * (count - 1))))
    }

    function requestPaint() {
      if (lineCanvas) lineCanvas.requestPaint()
    }

    implicitHeight: Math.max(Style.space(216), Math.min(Style.space(root.calendarLens ? 250 : 320), width * 0.34))
    radius: 0
    color: root.noFill

    activeFocusOnTab: visible
    onActiveFocusChanged: if (activeFocus) { selectedIndex = Math.max(0, selectedIndex); root.revealControl(lineRoot) }
    Keys.onLeftPressed: selectedIndex = Math.max(0, selectedIndex - 1)
    Keys.onRightPressed: selectedIndex = Math.min(days.length - 1, selectedIndex + 1)
    Accessible.role: Accessible.Button
    Accessible.name: selectedIndex >= 0 && selectedIndex < days.length
      ? Model.trendDetailText(days[selectedIndex]) + ". " + Model.cellActionText(days[selectedIndex]) : title + ". Use arrow keys to select a period"
    Keys.onReturnPressed: if (selectedIndex >= 0 && selectedIndex < days.length) activatedCell(days[selectedIndex])
    Keys.onSpacePressed: if (selectedIndex >= 0 && selectedIndex < days.length) activatedCell(days[selectedIndex])
    border.width: activeFocus ? 1 : 0
    border.color: root.accent
    onDaysChanged: { selectedIndex = Math.min(selectedIndex, days.length - 1); hoveredIndex = Math.min(hoveredIndex, days.length - 1); requestPaint() }
    Connections { target: root; function onViewKeyChanged() { lineRoot.selectedIndex = -1; lineRoot.hoveredIndex = -1 } }
    onMaxSecondsChanged: requestPaint()
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
        visible: lineRoot.inspectedIndex < 0
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: lineDetail
        visible: lineRoot.inspectedIndex < 0

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
      anchors.leftMargin: Style.space(52)
      anchors.topMargin: Style.space(8)
      anchors.bottomMargin: Style.space(26)

      Repeater {
        model: [1, 0.5, 0]
        Label {
          required property real modelData
          x: -Style.space(50)
          y: lineRoot.pointY(lineRoot.maxSeconds * modelData) - height / 2
          visible: lineRoot.hasActivity || modelData === 0
          width: Style.space(44)
          text: Model.fmt(lineRoot.maxSeconds * modelData)
          horizontalAlignment: Text.AlignRight
          color: root.dim
          font.pixelSize: Style.font.caption
        }
      }
      Label {
        anchors.right: parent.right
        visible: lineRoot.hasActivity && lineRoot.inspectedIndex < 0
        y: Math.max(0, lineRoot.pointY(lineRoot.averageSeconds) - height)
        z: 1
        text: "Average " + Model.fmt(lineRoot.averageSeconds)
        color: root.dim
        font.pixelSize: Style.font.caption
      }
      Repeater {
        model: 3

        Rectangle {
          required property int index

          anchors.left: parent.left
          anchors.right: parent.right
          y: Math.round(lineRoot.pointY(lineRoot.maxSeconds * (1 - (index + 1) / 4)))
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

        // Canvas pixels do not follow QML color bindings until repainted.
        Connections {
          target: root
          function onAccentChanged() { lineCanvas.requestPaint() }
          function onForegroundChanged() { lineCanvas.requestPaint() }
          function onRichGraphsChanged() { lineCanvas.requestPaint() }
        }

        anchors.fill: parent
        antialiasing: true
        opacity: root.richGraphs ? 0.72 + 0.28 * root.graphProgress : 1
        transform: Scale {
          origin.x: lineCanvas.width / 2; origin.y: lineCanvas.height
          xScale: 1
          yScale: root.richGraphs ? 0.96 + 0.04 * root.graphProgress : 1
        }

        onPaint: {
          var ctx = getContext("2d")
          ctx.reset()
          var list = lineRoot.days || []
          if (list.length <= 0 || lineRoot.maxSeconds <= 0 || width <= 0 || height <= 0) return

          var accent = root.accent
          var mutedAccent = root.withAlpha(root.accent, 0.26)
          var area = ctx.createLinearGradient(0, 0, 0, height)
          area.addColorStop(0, root.canvasColor(accent, root.richGraphs ? 0.40 : 0.28))
          area.addColorStop(1, root.canvasColor(accent, 0.03))

          ctx.beginPath()
          ctx.moveTo(lineRoot.pointX(0), height)
          lineRoot.traceCurve(ctx, "seconds", true)
          ctx.lineTo(lineRoot.pointX(list.length - 1), height)
          ctx.closePath()
          ctx.fillStyle = area
          ctx.fill()

          ctx.beginPath()
          lineRoot.traceCurve(ctx, "seconds", false)
          ctx.strokeStyle = root.canvasColor(accent, 0.92)
          ctx.lineWidth = Style.space(root.richGraphs ? 2.5 : 3)
          ctx.lineJoin = "round"
          ctx.lineCap = "round"
          ctx.stroke()

          if (list.some(function(d) { return Number(d.multitasked_seconds || 0) > 0 })) {
            ctx.beginPath()
            lineRoot.traceCurve(ctx, "multitasked_seconds", false)
            ctx.strokeStyle = root.canvasColor(root.foreground, 0.85)
            ctx.lineWidth = Style.space(2)
            ctx.stroke()
          }

          var count = list.length
          var dotStride = count <= 14 ? 1 : (count <= 31 ? 3 : Math.ceil(count / 12))
          for (var k = 0; k < count; k++) {
            var seconds = Number(list[k].seconds || 0)
            var partial = lineRoot.isCurrentDay(list[k])
            if (!partial && seconds <= 0 && k % dotStride !== 0) continue
            if (!partial && k % dotStride !== 0 && k !== 0 && k !== count - 1) continue
            var px = lineRoot.pointX(k)
            var py = lineRoot.pointY(seconds)
            ctx.beginPath()
            ctx.arc(px, py, Style.space(3), 0, Math.PI * 2, false)
            ctx.fillStyle = root.canvasColor(accent, count === 1 ? 0.92 : mutedAccent.a)
            if (!partial) ctx.fill()
            ctx.lineWidth = partial ? Style.space(2) : 0
            if (partial) {
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

      Rectangle {
        visible: !!lineRoot.inspectedDay
        x: lineRoot.pointX(lineRoot.inspectedIndex) - width / 2
        y: lineRoot.pointY(lineRoot.inspectedDay ? lineRoot.inspectedDay.seconds : 0) - height / 2
        width: Style.space(9); height: width; radius: width / 2
        color: root.foreground; border.width: Style.space(2); border.color: root.accent
      }
      Rectangle {
        visible: !!lineRoot.inspectedDay && Number(lineRoot.inspectedDay.multitasked_seconds || 0) > 0
        x: lineRoot.pointX(lineRoot.inspectedIndex) - width / 2
        y: lineRoot.pointY(lineRoot.inspectedDay ? lineRoot.inspectedDay.multitasked_seconds : 0) - height / 2
        width: Style.space(7); height: width; radius: width / 2
        color: root.foreground
      }

      Rectangle {
        width: Math.min(parent.width, hoverReadout.implicitWidth + Style.space(16))
        height: hoverReadout.implicitHeight + Style.space(10)
        x: Math.max(0, Math.min(parent.width - width, lineRoot.pointX(lineRoot.inspectedIndex) - width / 2))
        y: -height - Style.space(3)
        opacity: lineRoot.inspectedDay ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic } }
        radius: Style.space(root.richGraphs ? 10 : 4)
        color: root.foreground
        Label {
          id: hoverReadout
          anchors.centerIn: parent
          width: parent.width - Style.space(16)
          text: lineRoot.inspectedDay ? Model.chartDateLabel(lineRoot.inspectedDay) + " · " + Model.fmtPrecise(lineRoot.inspectedDay.seconds) + (lineRoot.isCurrentDay(lineRoot.inspectedDay) ? " so far" : "") : ""
          color: Color.background
          font.pixelSize: Style.font.caption
          wrapMode: Text.NoWrap
          elide: Text.ElideRight
        }
      }

      Repeater {
        model: Model.trendAxisTicks(lineRoot.days, linePlot.width / Style.space(1))
        Label {
          required property var modelData
          readonly property real tickWidth: Math.min(Style.space(180), linePlot.width / Math.max(1, modelData.count))
          width: tickWidth
          x: Math.max(0, Math.min(linePlot.width - width, lineRoot.pointX(modelData.index) - width / 2))
          y: linePlot.height + Style.space(5)
          text: Model.chartDateLabel(lineRoot.days[modelData.index]) + (lineRoot.isCurrentDay(lineRoot.days[modelData.index]) ? " · so far" : "")
          horizontalAlignment: modelData.index === 0 ? Text.AlignLeft : modelData.index === lineRoot.days.length - 1 ? Text.AlignRight : Text.AlignHCenter
          color: root.faint
          font.pixelSize: Style.font.caption
          wrapMode: Text.NoWrap
          elide: Text.ElideRight
        }
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
      reservedLines: 2
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
    property bool weeklyCells: false
    property real maxSeconds: 0
    property real weekMaxSeconds: 0
    property real weekdayMaxSeconds: 0
    property int selectedIndex: -1
    property int hoveredIndex: -1
    property string hoveredText: ""
    property real revealProgress: 1
    signal activatedCell(var cell)
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < cells.length
      ? Model.monthCellDetailText(cells[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property string defaultText: Model.monthDefaultText(cells, weeklyCells)
    readonly property bool compact: width > 0 && width < Style.space(540)
    readonly property real gap: Style.space(4)
    readonly property int rowCount: Math.ceil(cells.length / 7)
    readonly property real cellSize: Math.max(Style.space(22), Math.min(Style.space(weeklyCells ? 38 : 44), (width - Style.space(compact ? 24 : 284) - 6 * gap) / 7))
    readonly property real calendarWidth: 7 * cellSize + 6 * gap
    readonly property real calendarHeight: Style.space(40) + Style.space(4) + rowCount * cellSize + Math.max(0, rowCount - 1) * gap
    readonly property real weeklyPaceHeight: Style.space(18) + Style.space(5) + weeks.length * Style.space(22) + Math.max(0, weeks.length - 1) * Style.space(5)
    readonly property real sideHeight: weeklyPaceHeight + Style.space(10) + Style.space(54)
    readonly property real bodyHeight: compact ? calendarHeight + Style.space(10) + sideHeight : Math.max(calendarHeight, sideHeight)

    function settleReveal() {
      monthRhythmReveal.stop()
      revealProgress = 1
    }

    function restartReveal() {
      if (!visible || !root.opened || root.motionDuration === 0) {
        settleReveal()
        return
      }
      monthRhythmReveal.restart()
    }

    Connections {
      target: root
      function onViewRevealed() { monthRhythmRoot.restartReveal() }
      function onOpenedChanged() { if (!root.opened) monthRhythmRoot.settleReveal() }
      function onMotionDurationChanged() { if (root.motionDuration === 0) monthRhythmRoot.settleReveal() }
    }

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
      duration: root.motionDuration > 0 ? 180 : 0
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

        Label { text: monthRhythmRoot.weeklyCells ? "Weeks" : "Calendar days"; color: root.dim; font.pixelSize: Style.font.caption; font.bold: true }
        Row {
          width: monthRhythmRoot.calendarWidth
          height: monthRhythmRoot.weeklyCells ? 0 : implicitHeight
          visible: !monthRhythmRoot.weeklyCells
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
              readonly property color heatBase: root.accent

              readonly property bool canOpen: Model.cellDestination(modelData, Model.dateKey(new Date())) !== null
              activeFocusOnTab: canOpen
              onActiveFocusChanged: if (activeFocus) { monthRhythmRoot.selectedIndex = index; root.revealControl(this) }
              Keys.onReturnPressed: if (canOpen) monthRhythmRoot.activatedCell(modelData)
              Keys.onSpacePressed: if (canOpen) monthRhythmRoot.activatedCell(modelData)
              Accessible.role: Accessible.Button
              Accessible.name: Model.monthCellDetailText(modelData) + (canOpen ? (monthRhythmRoot.weeklyCells ? ". Open week view" : ". Open day view") : "")
              width: monthRhythmRoot.cellSize
              height: width
              radius: Style.space(4)
              color: modelData.blank
                ? "transparent"
                : (cellSeconds > 0
                  ? root.withAlpha(heatBase, 0.10 + monthRhythmRoot.revealProgress * (0.14 + 0.70 * cellIntensity))
                  : root.track)
              border.color: modelData.blank ? "transparent" : root.line
              border.width: activeFocus ? 2 : modelData.blank ? 0 : 1
              scale: (monthRhythmRoot.hoveredIndex === index || monthRhythmRoot.selectedIndex === index) && !modelData.blank ? 1.05 : 1.0

              Behavior on color {
                ColorAnimation { duration: root.motionDuration }
              }

              Behavior on scale {
                NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic }
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
                enabled: parent.canOpen
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
            text: monthRhythmRoot.weeklyCells ? "Monthly totals" : "Weekly totals"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            elide: Text.ElideRight
          }

          Repeater {
            model: weeks

            Controls.AbstractButton {
              id: periodTotal
              required property var modelData
              readonly property bool canOpen: Model.cellDestination(modelData, Model.dateKey(new Date())) !== null
              activeFocusOnTab: canOpen
              enabled: canOpen
              Accessible.name: Model.chartDateLabel(modelData) + ": " + Model.fmtPrecise(modelData.seconds) + ". " + Model.cellActionText(modelData)
              onActiveFocusChanged: if (activeFocus) root.revealControl(periodTotal)
              onClicked: monthRhythmRoot.activatedCell(modelData)
              Controls.ToolTip.visible: hovered || activeFocus
              Controls.ToolTip.text: Accessible.name
              background: Rectangle {
                radius: Style.space(4)
                color: periodTotal.hovered || periodTotal.activeFocus ? root.fill : "transparent"
                border.width: periodTotal.activeFocus ? 1 : 0
                border.color: root.accent
              }
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
                text: String(modelData.valueText || Model.fmt(modelData.seconds || 0))
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
                  color: root.withAlpha(root.accent, 0.9)
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
            text: "Weekday totals"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }

          Repeater {
            model: weekdays

            Controls.AbstractButton {
              id: weekdayTotal
              required property int index
              required property var modelData
              activeFocusOnTab: true
              Accessible.name: String(modelData.label) + ": " + Model.fmt(modelData.seconds)
              onActiveFocusChanged: if (activeFocus) root.revealControl(weekdayTotal)
              onClicked: monthRhythmRoot.hoveredText = Accessible.name
              Controls.ToolTip.visible: hovered || activeFocus
              Controls.ToolTip.text: Accessible.name
              background: Rectangle { color: "transparent"; border.width: weekdayTotal.activeFocus ? 1 : 0; border.color: root.accent }
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
                  height: Number(modelData.seconds || 0) > 0 ? Math.max(Style.space(3), parent.height * root.clamp01(Number(modelData.seconds) / Math.max(1, monthRhythmRoot.weekdayMaxSeconds)) * monthRhythmRoot.revealProgress) : 0
                  radius: parent.radius
                  color: root.withAlpha(root.accent, 0.82)
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
    property int reservedLines: 0

    property string text: ""

    visible: text.length > 0
    implicitHeight: visible ? Math.max(readoutLabel.implicitHeight, reservedLines * Style.font.caption * 1.4) + Style.space(8) : 0
    height: implicitHeight
    radius: Style.space(5)
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08)
    border.width: 0
    clip: true
    opacity: visible ? 1 : 0
    scale: visible ? 1 : 0.98

    Behavior on opacity {
      NumberAnimation { duration: root.motionDuration }
    }

    Behavior on scale {
      NumberAnimation { duration: root.motionDuration; easing.type: Easing.OutCubic }
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
      wrapMode: Text.WordWrap
    }
  }

}
