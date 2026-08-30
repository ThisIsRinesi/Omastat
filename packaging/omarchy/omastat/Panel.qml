import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

Panel {
  id: root
  moduleName: "local.omastat"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  property string selectedLens: "day"
  property int selectedOffset: 0
  property bool refreshRunning: false
  property var rows: []
  property var reportApps: []
  property var browserActivity: []
  property var summaryTopApp: null
  property var reportInsights: []
  property var widgetInsight: null
  property var daily: []
  property var heatmap: []
  property bool panelDataLoaded: false
  property string todayKey: ""
  property string lensLabel: "DAY"
  property string periodLabel: "Today"
  property int totalFocused: 0
  property int totalOpen: 0
  property int totalElapsed: 0
  property int totalObserved: 0
  property int totalIdle: 0
  property int totalLocked: 0
  property int totalSleep: 0
  property int totalUnobserved: 0
  property string statusText: ""
  property string errorText: ""
  property string updatedText: ""
  property int inspectedActivityIndex: -1
  property int inspectedHeatIndex: -1
  property int inspectedHourIndex: -1

  readonly property var barIdentity: hostWidget || root
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property color accent: bar ? bar.urgent : Color.accent
  readonly property color dim: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.58)
  readonly property color faint: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.36)
  readonly property color track: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.10)
  readonly property color fill: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.06)
  readonly property color line: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.18)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property var availableLenses: [
    { label: "Day", lens: "day" },
    { label: "Week", lens: "week" },
    { label: "Month", lens: "month" },
    { label: "Year", lens: "year" },
    { label: "Life", lens: "life" }
  ]
  readonly property bool compactPanel: panel.width > 0 && panel.width < Style.space(560)
  readonly property bool narrowPanel: panel.width > 0 && panel.width < Style.space(430)
  readonly property int tileColumns: panel.width > 0 && panel.width < Style.space(420) ? 1 : (compactPanel ? 2 : 4)
  readonly property int metricColumns: panel.width > 0 && panel.width < Style.space(420) ? 1 : (compactPanel ? 2 : 3)
  readonly property bool periodCanShift: selectedLens !== "life"
  readonly property string displayTodayKey: selectedOffset === 0 ? todayKey : ""
  readonly property int totalPaused: totalIdle + totalLocked + totalSleep
  readonly property int totalExcluded: totalPaused + totalUnobserved
  readonly property int focusDenominator: totalObserved > 0 ? totalObserved : Math.max(0, totalFocused + totalPaused)
  readonly property string focusShareText: focusDenominator > 0 ? Model.percent(totalFocused / focusDenominator) : "--"
  readonly property string observedDetailText: totalElapsed > 0 ? "Of " + root.formatDuration(totalElapsed) + " elapsed" : "Tracker-visible time"
  readonly property string pausedDetailText: Model.pausedDetail(totalIdle, totalLocked, totalSleep)
  readonly property string excludedDetailText: Model.excludedDetail(totalIdle, totalLocked, totalSleep, totalUnobserved)
  readonly property var visibleApps: reportApps && reportApps.length > 0 ? reportApps : Model.groupedApps(Model.appList(rows), Model.DONUT_MAX_SLICES)
  readonly property var visibleBrowserActivity: Model.browserActivity(browserActivity, 6)
  readonly property var topVisibleApp: visibleApps.length > 0 ? visibleApps[0] : null
  readonly property string topAppName: topVisibleApp ? String(topVisibleApp.app || "App") : "--"
  readonly property string topAppValue: topVisibleApp ? root.formatDuration(Number(topVisibleApp.seconds || 0)) + "  " + Number(topVisibleApp.pct || 0) + "%" : "--"
  readonly property var sliceColors: Model.sliceColors(visibleApps.length, Color.accent)
  readonly property var appColors: Model.stableAppColors(visibleApps, Color.accent)
  readonly property var monthCells: Model.monthCells(daily, selectedLens)
  readonly property var monthWeeks: Model.monthWeekCells(daily)
  readonly property var weekdayCells: Model.weekdayFocusCells(heatmap)
  readonly property var activityCells: Model.activityCells(daily, selectedLens)
  readonly property var trendLineCells: selectedLens === "week" ? Model.cumulativeCells(activityCells) : activityCells
  readonly property real activityMax: Model.maxDailySeconds(trendLineCells)
  readonly property real monthMax: Model.maxMonthSeconds(monthCells)
  readonly property real monthWeekMax: Model.maxDailySeconds(monthWeeks)
  readonly property real weekdayMax: Model.maxDailySeconds(weekdayCells)
  readonly property var heatCells: Model.heatmapCells(heatmap)
  readonly property var hourlyCells: Model.hourlyCells(heatmap)
  readonly property var hourlyTrendCells: Model.hourlyTrendCells(heatmap)
  readonly property var peakHour: Model.bestHour(hourlyTrendCells)
  readonly property real heatMax: Model.maxHeatSeconds(heatCells)
  readonly property real hourlyMax: Model.maxHourlySeconds(hourlyCells)
  readonly property var consistency: Model.consistencyStats(daily)
  readonly property var baseInsightRows: reportInsights && reportInsights.length > 0 ? reportInsights : Model.insights(rows, daily, todayKey, totalFocused)
  readonly property var insightRows: Model.enrichedInsights(baseInsightRows, visibleApps, daily, heatmap, selectedLens, totalFocused, totalElapsed)
  readonly property var usualPace: Model.usualPace(insightRows)
  readonly property var nowHabit: Model.nowHabit(insightRows)
  readonly property var insightGroups: Model.insightGroups(insightRows)
  readonly property bool hasFocusedData: totalFocused > 0 || visibleApps.length > 0
  readonly property bool showBreakdown: totalFocused + totalObserved + totalExcluded > 0
  readonly property real targetPanelWidth: Screen.width > 0 ? Math.min(Screen.width * 0.75, Style.space(1180)) : Style.space(1080)
  readonly property real targetPanelHeight: Screen.height > 0 ? Math.min(Screen.height * 0.88, Style.space(980)) : Style.space(820)
  readonly property bool widePanel: panel.width >= Style.space(900)
  readonly property bool showActivityChart: selectedLens === "month" ? monthMax > 0 : activityMax > 0
  readonly property bool showHeatmapChart: selectedLens !== "day" && heatMax > 0
  readonly property bool showHourlyChart: selectedLens === "day" && hourlyMax > 0
  readonly property string periodScopeLabel: selectedOffset === 0 && selectedLens !== "day" && selectedLens !== "life" ? periodLabel + " to date" : periodLabel
  readonly property string consistencyScopeText: selectedLens === "life" ? "Recent visible days" : (selectedOffset === 0 && selectedLens !== "day" ? "Elapsed days only" : "Across period")
  readonly property string loadingAppMixText: summaryTopApp && summaryTopApp.app ? "Loading app mix; top " + String(summaryTopApp.app) + " " + root.formatDuration(Number(summaryTopApp.seconds || 0)) : "Loading app mix..."
  readonly property string activityChartTitle: selectedLens === "day" ? "Last 7 days" : (selectedLens === "week" ? "This week" : (selectedLens === "month" ? "Month calendar" : (selectedLens === "year" ? "Monthly focus" : "Recent weeks")))
  readonly property string timeChartTitle: selectedLens === "day" ? "Today by hour" : "Focus by time of week"
  readonly property string activityChartDetail: selectedLens === "month" ? "Daily focused time" : (selectedLens === "week" ? "Cumulative focused time" : "Focused time")
  readonly property string timeChartDetail: selectedLens === "day" ? "Hourly focused time" : "Weekday and hour intensity"

  onSelectedLensChanged: clearInspection()
  onSelectedOffsetChanged: clearInspection()
  onDailyChanged: inspectedActivityIndex = -1
  onHeatmapChanged: {
    inspectedHeatIndex = -1
    inspectedHourIndex = -1
  }

  function refresh() {
    if (hostWidget && hostWidget.refresh) hostWidget.refresh(true)
  }

  IpcHandler {
    target: root.moduleName

    function open() {
      root.open()
      root.refresh()
    }
    function close() { root.close() }
    function show() {
      root.open()
      root.refresh()
    }
    function hide() { root.close() }
    function toggle() {
      root.toggle()
      if (root.opened) root.refresh()
    }
    function refresh() { root.refresh() }
    function status() { return root.statusText || "idle" }
    function day() { root.setLens("day") }
    function week() { root.setLens("week") }
    function month() { root.setLens("month") }
    function year() { root.setLens("year") }
    function life() { root.setLens("life") }
  }

  function setLens(lens) {
    if (hostWidget && hostWidget.setPeriod) hostWidget.setPeriod(lens, 0)
  }

  function shiftPeriod(delta) {
    if (!periodCanShift) return
    if (hostWidget && hostWidget.shiftPeriod) hostWidget.shiftPeriod(delta)
  }

  function formatDuration(seconds) {
    return Model.fmt(seconds)
  }

  function compactLensLabel(label) {
    return root.narrowPanel ? String(label || "").substr(0, 1) : String(label || "")
  }

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

  function toneColor(tone) {
    var value = String(tone || "")
    if (value === "positive") return root.sliceColor(1, 1.0)
    if (value === "negative" || value === "caution") return Color.urgent
    if (value === "info") return root.sliceColor(2, 1.0)
    return root.sliceColor(0, 1.0)
  }

  function scrollBy(dy) {
    if (scroll.contentHeight <= scroll.height) return
    scroll.contentY = Math.max(0, Math.min(scroll.contentHeight - scroll.height, scroll.contentY + dy))
  }

  function clearInspection() {
    inspectedActivityIndex = -1
    inspectedHeatIndex = -1
    inspectedHourIndex = -1
  }

  function inspectActivity(delta) {
    var count = root.selectedLens === "month" ? root.monthCells.length : root.activityCells.length
    if (count <= 0) return

    var direction = delta < 0 ? -1 : 1
    var next = inspectedActivityIndex >= 0
      ? inspectedActivityIndex + direction
      : (direction > 0 ? 0 : count - 1)
    for (var i = 0; i < count; i++) {
      next = (next + count) % count
      if (root.selectedLens !== "month" || !(root.monthCells[next] && root.monthCells[next].blank)) {
        inspectedActivityIndex = next
        return
      }
      next += direction
    }
  }

  function inspectHeat(delta) {
    var count = root.heatCells.length
    if (count <= 0) return
    var direction = delta < 0 ? -1 : 1
    var next = inspectedHeatIndex >= 0
      ? inspectedHeatIndex + direction
      : (direction > 0 ? 0 : count - 1)
    inspectedHeatIndex = (next + count) % count
  }

  function inspectTime(delta) {
    if (root.showHourlyChart) {
      root.inspectHour(delta)
      return
    }
    root.inspectHeat(delta)
  }

  function inspectHour(delta) {
    var count = root.hourlyTrendCells.length
    if (count <= 0) return
    var direction = delta < 0 ? -1 : 1
    var next = inspectedHourIndex >= 0
      ? inspectedHourIndex + direction
      : (direction > 0 ? 0 : count - 1)
    inspectedHourIndex = (next + count) % count
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    margin: Math.max(Style.gapsOut, Style.space(12))
    gap: Math.max(Style.gapsOut, Style.space(8))
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(root.targetPanelWidth)
    contentHeight: panel.fittedContentHeight(headerRow.implicitHeight + Style.space(12) + contentColumn.implicitHeight, root.targetPanelHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        if (dx !== 0) root.inspectActivity(dx)
        if (dy !== 0) root.scrollBy(-dy * Style.space(28))
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(text) {
        if (text === "r" || text === "R") root.refresh()
        else if (text === "1") root.setLens("day")
        else if (text === "2") root.setLens("week")
        else if (text === "3") root.setLens("month")
        else if (text === "4") root.setLens("year")
        else if (text === "5") root.setLens("life")
        else if (text === "[") root.shiftPeriod(-1)
        else if (text === "]") root.shiftPeriod(1)
        else if (text === "h" || text === "H") root.inspectTime(-1)
        else if (text === "l" || text === "L") root.inspectTime(1)
        else if (text === "Escape") root.clearInspection()
      }

      Item {
        id: headerRow
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        implicitHeight: Math.max(heroColumn.implicitHeight, headerActions.implicitHeight)

        Row {
          id: heroColumn
          anchors.left: parent.left
          anchors.right: headerActions.left
          anchors.rightMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(12)

          Rectangle {
            width: Style.space(42)
            height: width
            visible: !root.narrowPanel
            radius: Style.space(8)
            color: root.fill
            border.color: root.line
            border.width: 1
            anchors.verticalCenter: parent.verticalCenter

            Text {
              anchors.centerIn: parent
              text: "󰔟"
              color: root.sliceColor(0, 1.0)
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
            }
          }

          Column {
            width: Math.max(0, parent.width - (root.narrowPanel ? 0 : Style.space(54)))
            spacing: Style.space(2)
            anchors.verticalCenter: parent.verticalCenter

            Text {
              width: parent.width
              text: root.periodScopeLabel
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              elide: Text.ElideRight
            }

            Text {
              width: parent.width
              text: root.topVisibleApp ? root.topAppName + " leads with " + root.topAppValue : "No focused app time yet"
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: true
              elide: Text.ElideRight
            }
          }
        }

        Row {
          id: headerActions
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(6)

          PanelActionButton {
            enabled: root.periodCanShift
            iconText: "<"
            tooltipText: "Previous period"
            foreground: root.foreground
            hoverColor: root.sliceColor(1, 1.0)
            fontFamily: root.fontFamily
            bordered: true
            onClicked: root.shiftPeriod(-1)
          }

          PanelActionButton {
            enabled: root.periodCanShift && root.selectedOffset < 0
            iconText: ">"
            tooltipText: "Next period"
            foreground: root.foreground
            hoverColor: root.sliceColor(1, 1.0)
            fontFamily: root.fontFamily
            bordered: true
            onClicked: root.shiftPeriod(1)
          }

          PanelActionButton {
            id: refreshButton

            iconText: "󰑐"
            tooltipText: "Refresh"
            foreground: root.foreground
            hoverColor: root.sliceColor(0, 1.0)
            fontFamily: root.fontFamily
            bordered: true
            opacity: root.refreshRunning ? 0.74 : 1.0
            onClicked: root.refresh()

            Behavior on opacity {
              NumberAnimation { duration: 140 }
            }

            NumberAnimation on rotation {
              running: root.refreshRunning
              loops: Animation.Infinite
              from: 0
              to: 360
              duration: 900
            }
          }
        }

        Rectangle {
          anchors.left: parent.left
          anchors.right: parent.right
          anchors.bottom: parent.bottom
          height: Style.space(2)
          radius: height / 2
          color: root.sliceColor(0, 0.85)
          opacity: root.refreshRunning ? 1 : 0
          scale: root.refreshRunning ? 1 : 0
          transformOrigin: Item.Left

          Behavior on opacity {
            NumberAnimation { duration: 140 }
          }

          Behavior on scale {
            NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
          }
        }
      }

      Flickable {
        id: scroll
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: headerRow.bottom
        anchors.topMargin: Style.space(12)
        anchors.bottom: parent.bottom
        contentWidth: contentColumn.width
        contentHeight: contentColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: contentColumn
          width: scroll.width
          spacing: Style.space(12)
          opacity: root.opened ? 1 : 0
          scale: root.opened ? 1.0 : 0.985
          transformOrigin: Item.Top

          Behavior on opacity {
            NumberAnimation { duration: 180; easing.type: Easing.OutCubic }
          }

          Behavior on scale {
            NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
          }

          Row {
            width: parent.width
            spacing: Style.space(8)

            Repeater {
              model: root.availableLenses

              LensTab {
                required property var modelData

                width: root.availableLenses.length > 0
                  ? (parent.width - parent.spacing * (root.availableLenses.length - 1)) / root.availableLenses.length
                  : 0
                label: root.compactLensLabel(modelData.label)
                lens: String(modelData.lens || "day")
                selected: root.selectedLens === lens
                onSelectedLens: function(lens) { root.setLens(lens) }
              }
            }
          }

          DashboardSummary {
            width: parent.width
          }

          GridLayout {
            id: primaryAnalyticsGrid

            width: parent.width
            visible: root.showActivityChart || root.showHourlyChart || root.showHeatmapChart
            columns: root.widePanel && (root.showActivityChart && (root.showHourlyChart || root.showHeatmapChart)) ? 2 : 1
            rowSpacing: Style.space(12)
            columnSpacing: Style.space(12)

            Column {
              Layout.fillWidth: true
              Layout.preferredWidth: primaryAnalyticsGrid.columns > 1 ? Math.max(0, (primaryAnalyticsGrid.width - primaryAnalyticsGrid.columnSpacing) / 2) : primaryAnalyticsGrid.width
              Layout.alignment: Qt.AlignTop
              visible: root.showActivityChart
              spacing: Style.space(8)

              FocusTrendLine {
                width: parent.width
                expanded: root.widePanel
                visible: root.selectedLens !== "month" && root.activityMax > 0
                title: root.activityChartTitle
                detail: root.activityChartDetail
                days: root.trendLineCells
                maxSeconds: root.activityMax
                selectedIndex: root.inspectedActivityIndex
              }

              MonthRhythm {
                width: parent.width
                visible: root.selectedLens === "month" && root.monthCells.length > 0
                title: root.activityChartTitle
                detail: root.activityChartDetail
                cells: root.monthCells
                weeks: root.monthWeeks
                weekdays: root.weekdayCells
                maxSeconds: root.monthMax
                weekMaxSeconds: root.monthWeekMax
                weekdayMaxSeconds: root.weekdayMax
                selectedIndex: root.inspectedActivityIndex
              }
            }

            Column {
              Layout.fillWidth: true
              Layout.preferredWidth: primaryAnalyticsGrid.columns > 1 ? Math.max(0, (primaryAnalyticsGrid.width - primaryAnalyticsGrid.columnSpacing) / 2) : primaryAnalyticsGrid.width
              Layout.alignment: Qt.AlignTop
              visible: root.showHourlyChart || root.showHeatmapChart
              spacing: Style.space(8)

              FocusRing {
                width: parent.width
                expanded: root.widePanel
                visible: root.showHourlyChart
                title: "24-hour focus ring"
                detail: root.timeChartDetail
                hours: root.hourlyTrendCells
                maxSeconds: root.hourlyMax
                selectedIndex: root.inspectedHourIndex
              }

              HeatmapGrid {
                width: parent.width
                expanded: root.widePanel
                visible: root.showHeatmapChart
                title: root.timeChartTitle
                detail: root.timeChartDetail
                cells: root.heatCells
                maxSeconds: root.heatMax
                selectedIndex: root.inspectedHeatIndex
              }
            }
          }

          SectionHeader {
            text: "Focus Mix"
          }

          Rectangle {
            id: appMixCard

            width: parent.width
            implicitHeight: appMixLayout.implicitHeight + Style.space(24)
            radius: Style.space(7)
            color: root.fill
            border.color: root.line
            border.width: 1
            visible: root.visibleApps.length > 0
            clip: true

            Rectangle {
              anchors.left: parent.left
              anchors.top: parent.top
              anchors.bottom: parent.bottom
              width: Style.space(3)
              color: root.sliceColor(0, 0.72)
              opacity: root.widePanel ? 1 : 0.72
            }

            GridLayout {
              id: appMixLayout

              anchors.left: parent.left
              anchors.right: parent.right
              anchors.top: parent.top
              anchors.margins: Style.space(12)
              columns: root.widePanel ? 2 : 1
              rowSpacing: Style.space(12)
              columnSpacing: Style.space(14)

              AppDonut {
                Layout.fillWidth: true
                Layout.preferredWidth: root.widePanel ? Math.max(Style.space(240), appMixCard.width * 0.34) : appMixCard.width
                apps: root.visibleApps
                colors: root.appColors
                totalSeconds: root.totalFocused
              }

              AppRankBars {
                id: appRankBars

                Layout.fillWidth: true
                Layout.preferredWidth: root.widePanel ? Math.max(0, appMixCard.width * 0.60) : appMixCard.width
                apps: root.visibleApps
                colors: root.appColors
              }
            }
          }

          BrowserFocus {
            width: parent.width
            visible: root.visibleBrowserActivity.length > 0
            rows: root.visibleBrowserActivity
          }

          EmptyState {
            visible: root.visibleApps.length === 0
            text: root.errorText !== "" ? root.errorText : (!root.panelDataLoaded && root.refreshRunning ? "Loading analytics..." : "No focused app time for this period")
            urgent: root.errorText !== ""
          }

          SectionHeader {
            text: root.selectedLens === "life" ? "Recent Consistency" : "Consistency"
            visible: root.daily.length > 0
          }

          ConsistencyMetrics {
            width: parent.width
            visible: root.daily.length > 0
            columnsValue: root.widePanel ? 4 : root.tileColumns
          }

          SectionHeader {
            text: "Insights"
            visible: root.insightGroups.length > 0
          }

          InsightLanes {
            width: parent.width
            visible: root.insightGroups.length > 0
            groups: root.insightGroups
          }

          SectionHeader {
            text: "Tracking Quality"
            visible: root.showBreakdown
          }

          TimeBreakdownStrip {
            width: parent.width
            visible: root.showBreakdown
            focusedSeconds: root.totalFocused
            observedSeconds: root.totalObserved
            pausedSeconds: root.totalPaused
            trackerOffSeconds: root.totalUnobserved
            focusedShare: root.focusDenominator > 0 ? root.totalFocused / root.focusDenominator : 0
            excludedDetail: root.excludedDetailText
          }

          Text {
            visible: root.errorText !== "" || root.refreshRunning
            width: parent.width
            text: root.errorText !== "" ? root.errorText : "Refreshing..."
            color: root.errorText !== "" ? Color.urgent : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
          }

          Text {
            visible: root.updatedText !== ""
            width: parent.width
            text: "Updated " + root.updatedText
            color: root.faint
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
          }
        }
      }

      Rectangle {
        anchors.top: scroll.top
        anchors.right: parent.right
        anchors.bottom: scroll.bottom
        width: Style.space(3)
        radius: width / 2
        color: root.track
        visible: scroll.contentHeight > scroll.height + Style.space(2)
        opacity: visible ? 1 : 0

        Rectangle {
          width: parent.width
          height: Math.max(Style.space(34), parent.height * scroll.height / Math.max(scroll.contentHeight, 1))
          y: (parent.height - height) * scroll.contentY / Math.max(1, scroll.contentHeight - scroll.height)
          radius: width / 2
          color: root.sliceColor(0, 0.70)

          Behavior on y {
            NumberAnimation { duration: 90; easing.type: Easing.OutCubic }
          }
        }
      }
    }
  }

  component LensTab: Rectangle {
    property string label: ""
    property string lens: "day"
    property bool selected: false
    signal selectedLens(string lens)

    height: Style.space(34)
    radius: Style.space(6)
    color: selected ? root.sliceColor(0, 0.22) : root.track
    border.color: selected ? root.sliceColor(0, 0.82) : root.line
    border.width: 1
    scale: selected ? 1.0 : 0.985

    Behavior on color {
      ColorAnimation { duration: 140 }
    }

    Behavior on border.color {
      ColorAnimation { duration: 140 }
    }

    Behavior on scale {
      NumberAnimation { duration: 120; easing.type: Easing.OutCubic }
    }

    Text {
      anchors.centerIn: parent
      text: label
      color: selected ? root.foreground : root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      font.bold: true
      elide: Text.ElideRight
    }

    MouseArea {
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: selectedLens(lens)
    }
  }

  component SectionHeader: Text {
    width: parent ? parent.width : implicitWidth
    color: root.dim
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    font.bold: true
    text: ""
    elide: Text.ElideRight
  }

  component DashboardSummary: Rectangle {
    id: summaryRoot

    readonly property int otherTrackedSeconds: Math.max(0, root.totalObserved - root.totalFocused - root.totalPaused)
    readonly property int elapsedSeconds: Math.max(1, root.totalObserved + root.totalUnobserved)

    implicitHeight: summaryColumn.implicitHeight + Style.space(24)
    radius: Style.space(7)
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.075)
    border.color: root.sliceColor(0, root.refreshRunning ? 0.56 : 0.28)
    border.width: 1
    clip: true

    Rectangle {
      anchors.left: parent.left
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      width: Style.space(3)
      color: root.errorText !== "" ? Color.urgent : root.sliceColor(0, 0.92)
    }

    Column {
      id: summaryColumn

      anchors.left: parent.left
      anchors.leftMargin: Style.space(16)
      anchors.right: parent.right
      anchors.rightMargin: Style.space(14)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(10)

      Item {
        width: parent.width
        height: Math.max(primaryValue.implicitHeight + primaryLabel.implicitHeight + Style.space(2), focusSharePill.implicitHeight)

        Column {
          anchors.left: parent.left
          anchors.right: focusSharePill.left
          anchors.rightMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(1)

          Text {
            id: primaryLabel

            width: parent.width
            text: root.periodScopeLabel
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            elide: Text.ElideRight
          }

          Text {
            id: primaryValue

            width: parent.width
            text: root.formatDuration(root.totalFocused)
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.title + Style.space(2)
            font.bold: true
            elide: Text.ElideRight
          }
        }

        Rectangle {
          id: focusSharePill

          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          implicitWidth: focusShareColumn.implicitWidth + Style.space(20)
          implicitHeight: focusShareColumn.implicitHeight + Style.space(10)
          radius: Style.space(6)
          color: root.sliceColor(1, 0.16)
          border.color: root.sliceColor(1, 0.36)
          border.width: 1

          Column {
            id: focusShareColumn

            anchors.centerIn: parent
            spacing: 0

            Text {
              text: root.peakHour.label
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              font.bold: true
              horizontalAlignment: Text.AlignHCenter
            }

            Text {
              text: "peak hour"
              color: root.faint
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              horizontalAlignment: Text.AlignHCenter
            }
          }
        }
      }

      GridLayout {
        width: parent.width
        columns: root.widePanel ? 4 : root.metricColumns
        rowSpacing: Style.space(8)
        columnSpacing: Style.space(12)

        SummaryStat {
          Layout.fillWidth: true
          label: root.usualPace.available ? root.usualPace.label : "Period"
          value: root.usualPace.available ? root.usualPace.value : root.periodScopeLabel
          detail: root.usualPace.available
            ? root.usualPace.detail
            : (root.selectedOffset === 0 ? "Current lens" : "Historical lens")
          accentColor: root.usualPace.available ? root.toneColor(root.usualPace.tone) : root.sliceColor(0, 1.0)
        }

        SummaryStat {
          Layout.fillWidth: true
          label: "Top app"
          value: root.topAppName
          detail: root.topAppValue
          accentColor: root.sliceColor(2, 1.0)
        }

        SummaryStat {
          Layout.fillWidth: true
          label: root.nowHabit.available ? root.nowHabit.label : "Peak time"
          value: root.nowHabit.available ? root.nowHabit.value : root.peakHour.label
          detail: root.nowHabit.available ? root.nowHabit.detail : root.peakHour.value + "  " + root.peakHour.detail
          accentColor: root.nowHabit.available ? root.toneColor(root.nowHabit.tone) : root.sliceColor(1, 1.0)
        }

        SummaryStat {
          Layout.fillWidth: true
          label: "Coverage"
          value: root.totalUnobserved > 0 ? root.formatDuration(root.totalUnobserved) + " gap" : "Complete"
          detail: root.totalUnobserved > 0
            ? root.excludedDetailText
            : (root.pausedDetailText.length > 0 ? "Paused " + root.pausedDetailText : "No excluded time")
          accentColor: root.totalUnobserved > 0 ? Color.urgent : root.sliceColor(1, 1.0)
        }
      }

      Row {
        width: parent.width
        height: Style.space(10)
        spacing: 0
        visible: root.showBreakdown
        clip: true

        Rectangle {
          width: parent.width * root.clamp01(root.totalFocused / summaryRoot.elapsedSeconds)
          height: parent.height
          radius: Style.space(4)
          color: root.sliceColor(0, 0.94)
        }

        Rectangle {
          width: parent.width * root.clamp01(root.totalPaused / summaryRoot.elapsedSeconds)
          height: parent.height
          color: root.sliceColor(3, 0.84)
        }

        Rectangle {
          width: parent.width * root.clamp01(summaryRoot.otherTrackedSeconds / summaryRoot.elapsedSeconds)
          height: parent.height
          color: root.sliceColor(1, 0.55)
        }

        Rectangle {
          width: parent.width * root.clamp01(root.totalUnobserved / summaryRoot.elapsedSeconds)
          height: parent.height
          radius: Style.space(4)
          color: root.totalUnobserved > 0 ? Color.urgent : root.faint
        }
      }
    }
  }

  component SummaryStat: Item {
    property string label: ""
    property string value: ""
    property string detail: ""
    property color accentColor: root.accent

    Layout.minimumWidth: Style.space(126)
    implicitHeight: Style.space(48)

    Rectangle {
      width: Style.space(6)
      height: width
      radius: width / 2
      color: accentColor
      anchors.left: parent.left
      anchors.top: parent.top
      anchors.topMargin: Style.space(5)
    }

    Column {
      anchors.left: parent.left
      anchors.leftMargin: Style.space(12)
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(1)

      Text {
        width: parent.width
        text: label
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: value
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }
    }
  }

  component MetricTile: Rectangle {
    property string label: ""
    property string value: ""
    property string detail: ""
    property color accentColor: root.accent

    Layout.minimumWidth: Style.space(118)
    implicitHeight: Style.space(76)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

    Rectangle {
      width: Style.space(3)
      height: parent.height - Style.space(18)
      radius: width / 2
      color: accentColor
      anchors.left: parent.left
      anchors.leftMargin: Style.space(9)
      anchors.verticalCenter: parent.verticalCenter
    }

    Column {
      anchors.left: parent.left
      anchors.leftMargin: Style.space(20)
      anchors.right: parent.right
      anchors.rightMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(2)

      Text {
        width: parent.width
        text: label
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: value
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.body
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }
    }
  }

  component ConsistencyMetrics: GridLayout {
    property int columnsValue: root.tileColumns

    columns: columnsValue
    rowSpacing: Style.space(8)
    columnSpacing: Style.space(8)

    MetricTile {
      Layout.fillWidth: true
      label: "Active Days"
      value: root.consistency.activeDays + " / " + root.consistency.totalDays
      detail: root.selectedLens === "life" ? "Recent days with focus" : "Days with focus"
      accentColor: root.sliceColor(0, 1.0)
    }

    MetricTile {
      Layout.fillWidth: true
      label: "Best Streak"
      value: root.consistency.longestStreak + "d"
      detail: "Consecutive days"
      accentColor: root.sliceColor(1, 1.0)
    }

    MetricTile {
      Layout.fillWidth: true
      label: "Daily Avg"
      value: root.formatDuration(root.consistency.dailyAverageSeconds)
      detail: root.consistencyScopeText
      accentColor: root.sliceColor(2, 1.0)
    }

    MetricTile {
      Layout.fillWidth: true
      label: "Best Day"
      value: root.consistency.bestDaySeconds > 0 ? root.formatDuration(root.consistency.bestDaySeconds) : "--"
      detail: root.consistency.bestDayLabel
      accentColor: root.sliceColor(3, 1.0)
    }
  }

  component TimeBreakdownStrip: Rectangle {
    id: breakdownRoot

    property int focusedSeconds: 0
    property int observedSeconds: 0
    property int pausedSeconds: 0
    property int trackerOffSeconds: 0
    property real focusedShare: 0
    property string excludedDetail: ""
    property real revealProgress: 0

    readonly property int otherTrackedSeconds: Math.max(0, observedSeconds - focusedSeconds - pausedSeconds)
    readonly property int elapsedSeconds: Math.max(1, observedSeconds + trackerOffSeconds)

    function restartReveal() {
      revealProgress = 0
      breakdownReveal.restart()
    }

    onFocusedSecondsChanged: restartReveal()
    onObservedSecondsChanged: restartReveal()
    onPausedSecondsChanged: restartReveal()
    onTrackerOffSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: Style.space(128)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

    NumberAnimation {
      id: breakdownReveal

      target: breakdownRoot
      property: "revealProgress"
      from: 0
      to: 1
      duration: 520
      easing.type: Easing.OutCubic
    }

    Column {
      anchors.fill: parent
      anchors.margins: Style.space(12)
      spacing: Style.space(8)

      Item {
        width: parent.width
        height: Style.space(22)

        Text {
          anchors.left: parent.left
          anchors.right: observedBreakdownLabel.left
          anchors.rightMargin: Style.space(10)
          anchors.verticalCenter: parent.verticalCenter
          text: "Elapsed breakdown"
          color: root.dim
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          font.bold: true
        }

        Text {
          id: observedBreakdownLabel

          width: Math.min(implicitWidth, parent.width * 0.52)
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: observedSeconds > 0 ? root.formatDuration(observedSeconds) + " observed" : root.observedDetailText
          color: root.faint
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }

      Row {
        width: parent.width
        height: Style.space(14)
        spacing: 0
        clip: true

        Rectangle {
          width: parent.width * root.clamp01(focusedSeconds / breakdownRoot.elapsedSeconds) * breakdownRoot.revealProgress
          height: parent.height
          radius: Style.space(4)
          color: root.sliceColor(0, 0.95)
        }

        Rectangle {
          width: parent.width * root.clamp01(pausedSeconds / breakdownRoot.elapsedSeconds) * breakdownRoot.revealProgress
          height: parent.height
          color: root.sliceColor(3, 0.95)
        }

        Rectangle {
          width: parent.width * root.clamp01(breakdownRoot.otherTrackedSeconds / breakdownRoot.elapsedSeconds) * breakdownRoot.revealProgress
          height: parent.height
          color: root.sliceColor(1, 0.65)
        }

        Rectangle {
          width: parent.width * root.clamp01(trackerOffSeconds / breakdownRoot.elapsedSeconds) * breakdownRoot.revealProgress
          height: parent.height
          radius: Style.space(4)
          color: trackerOffSeconds > 0 ? Color.urgent : root.faint
        }
      }

      GridLayout {
        width: parent.width
        columns: root.compactPanel ? 2 : 4
        rowSpacing: Style.space(6)
        columnSpacing: Style.space(8)

        BreakdownLegend { label: "Focused"; value: root.formatDuration(focusedSeconds) + (observedSeconds > 0 ? "  " + Model.percent(focusedShare) : ""); colorValue: root.sliceColor(0, 0.95) }
        BreakdownLegend { label: "Paused"; value: root.formatDuration(pausedSeconds); colorValue: root.sliceColor(3, 0.95) }
        BreakdownLegend { label: "Other Tracked"; value: root.formatDuration(breakdownRoot.otherTrackedSeconds); colorValue: root.sliceColor(1, 0.65) }
        BreakdownLegend { label: "Tracker Off"; value: root.formatDuration(trackerOffSeconds); colorValue: trackerOffSeconds > 0 ? Color.urgent : root.faint }
      }

      Text {
        width: parent.width
        text: root.excludedDetailText.length > 0 ? root.excludedDetailText : "No tracker gaps"
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }
    }
  }

  component BreakdownLegend: Item {
    property string label: ""
    property string value: ""
    property color colorValue: root.accent

    Layout.fillWidth: true
    implicitHeight: Style.space(24)

    Rectangle {
      width: Style.space(8)
      height: width
      radius: width / 2
      color: colorValue
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
    }

    Column {
      anchors.left: parent.left
      anchors.leftMargin: Style.space(14)
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      spacing: 0

      Text {
        width: parent.width
        text: label
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: value
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        elide: Text.ElideRight
      }
    }
  }

  component BreakdownBar: Item {
    property string label: ""
    property string value: ""
    property real ratio: 0
    property real revealProgress: 1
    property color colorValue: root.accent
    property string detail: ""

    implicitHeight: Style.space(detail.length > 0 ? 34 : 26)

    Rectangle {
      id: signalTrack

      anchors.left: parent.left
      anchors.right: valueLabel.left
      anchors.rightMargin: Style.space(10)
      anchors.bottom: parent.bottom
      height: Style.space(6)
      radius: height / 2
      color: root.track

      Rectangle {
        width: signalTrack.width * root.clamp01(ratio) * root.clamp01(revealProgress)
        height: parent.height
        radius: parent.radius
        color: colorValue

        Behavior on width {
          NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
        }
      }
    }

    Text {
      id: legendLabel
      anchors.left: parent.left
      anchors.right: valueLabel.left
      anchors.rightMargin: Style.space(10)
      anchors.top: parent.top
      text: label
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: true
      elide: Text.ElideRight
    }

    Text {
      id: valueLabel
      width: Math.min(implicitWidth, Style.space(96))
      anchors.right: parent.right
      anchors.top: parent.top
      text: value
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: true
      horizontalAlignment: Text.AlignRight
      elide: Text.ElideRight
    }

    Text {
      anchors.left: parent.left
      anchors.right: valueLabel.left
      anchors.rightMargin: Style.space(10)
      anchors.top: legendLabel.bottom
      visible: detail.length > 0
      text: detail
      color: root.faint
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
    }
  }

  component AppDonut: Item {
    id: donutRoot

    property var apps: []
    property var colors: []
    property int totalSeconds: 0
    property int hoveredIndex: -1

    readonly property var segments: Model.arcSegments(apps)
    readonly property string centerLabel: root.formatDuration(totalSeconds)
    readonly property string centerDetail: apps.length > 0 ? String(apps[0].app || "Top app") : "Focused"
    readonly property int chartSize: Math.min(Style.space(190), Math.max(Style.space(142), width - Style.space(24)))

    Layout.minimumHeight: Style.space(252)
    implicitHeight: Style.space(252)

    onAppsChanged: donutCanvas.requestPaint()
    onColorsChanged: donutCanvas.requestPaint()
    onHoveredIndexChanged: donutCanvas.requestPaint()
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
          text: "Top apps"
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
            ctx.lineCap = "round"

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
              var color = root.colorFromHex(String(donutRoot.colors[i] || Color.accent), i === donutRoot.hoveredIndex || donutRoot.hoveredIndex < 0 ? 0.94 : 0.46)
              ctx.beginPath()
              ctx.arc(cx, cy, radius, start, end, false)
              ctx.strokeStyle = root.canvasColor(color, color.a)
              ctx.lineWidth = i === donutRoot.hoveredIndex ? lineWidth + Style.space(3) : lineWidth
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
          onExited: donutRoot.hoveredIndex = -1
        }
      }

      Text {
        width: parent.width
        text: donutRoot.hoveredIndex >= 0 && donutRoot.hoveredIndex < donutRoot.apps.length
          ? String(donutRoot.apps[donutRoot.hoveredIndex].app || "App") + "  " + root.formatDuration(Number(donutRoot.apps[donutRoot.hoveredIndex].seconds || 0)) + "  " + Number(donutRoot.apps[donutRoot.hoveredIndex].pct || 0) + "%"
          : "Focused time share"
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
      }
    }
  }

  component AppRankBars: Column {
    id: rankRoot

    property var apps: []
    property var colors: []

    readonly property int maxSeconds: {
      var value = 0
      for (var i = 0; i < apps.length; i++) value = Math.max(value, Number(apps[i].seconds || 0))
      return Math.max(1, value)
    }

    spacing: Style.space(8)
    readonly property real contentHeight: rankHeader.implicitHeight + Style.space(8) + rankRows.implicitHeight

    Item {
      id: rankHeader
      width: parent.width
      height: Style.space(18)

      Text {
        anchors.left: parent.left
        anchors.right: rankHeaderValue.left
        anchors.rightMargin: Style.space(10)
        anchors.verticalCenter: parent.verticalCenter
        text: "Ranked apps"
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }

      Text {
        id: rankHeaderValue

        width: Math.min(implicitWidth, Style.space(112))
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: root.formatDuration(root.totalFocused)
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        elide: Text.ElideRight
      }
    }

    Column {
      id: rankRows
      width: parent.width
      spacing: Style.space(7)

      Repeater {
        model: apps

        Item {
          required property int index
          required property var modelData

          width: parent.width
          height: Style.space(34)

          readonly property int seconds: Number(modelData.seconds || 0)
          readonly property int pct: Number(modelData.pct || 0)
          readonly property color barColor: root.colorFromHex(String(colors[index] || Color.accent), index === 0 ? 0.88 : 0.68)

          Text {
            id: rankName
            anchors.left: parent.left
            anchors.right: rankValue.left
            anchors.rightMargin: Style.space(10)
            anchors.top: parent.top
            text: String(modelData.app || "")
            color: index === 0 ? root.foreground : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            font.bold: index === 0
            elide: Text.ElideRight
          }

          Text {
            id: rankValue
            width: Style.space(112)
            anchors.right: parent.right
            anchors.top: parent.top
            text: root.formatDuration(seconds) + "  " + pct + "%"
            color: root.foreground
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            font.bold: true
            horizontalAlignment: Text.AlignRight
            elide: Text.ElideRight
          }

          Rectangle {
            id: rankTrack
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: Style.space(7)
            radius: height / 2
            color: root.track

            Rectangle {
              width: rankTrack.width * root.clamp01(seconds / rankRoot.maxSeconds)
              height: parent.height
              radius: parent.radius
              color: barColor

              Behavior on width {
                NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
              }
            }
          }
        }
      }
    }
  }

  component BrowserFocus: Rectangle {
    id: browserRoot

    property var rows: []

    readonly property int totalSeconds: {
      var total = 0
      for (var i = 0; i < rows.length; i++) total += Number(rows[i].seconds || 0)
      return total
    }
    readonly property int maxSeconds: {
      var value = 0
      for (var i = 0; i < rows.length; i++) value = Math.max(value, Number(rows[i].seconds || 0))
      return Math.max(1, value)
    }

    implicitHeight: browserColumn.implicitHeight + Style.space(24)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1
    clip: true

    Rectangle {
      anchors.left: parent.left
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      width: Style.space(3)
      color: root.sliceColor(2, 0.78)
    }

    Column {
      id: browserColumn

      anchors.left: parent.left
      anchors.leftMargin: Style.space(14)
      anchors.right: parent.right
      anchors.rightMargin: Style.space(14)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(8)

      Item {
        width: parent.width
        height: Style.space(20)

        Text {
          anchors.left: parent.left
          anchors.right: browserTotal.left
          anchors.rightMargin: Style.space(10)
          anchors.verticalCenter: parent.verticalCenter
          text: "Browser Focus"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          font.bold: true
          elide: Text.ElideRight
        }

        Text {
          id: browserTotal

          width: Style.space(128)
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          text: root.formatDuration(browserRoot.totalSeconds)
          color: root.faint
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          horizontalAlignment: Text.AlignRight
          elide: Text.ElideRight
        }
      }

      Column {
        width: parent.width
        spacing: Style.space(7)

        Repeater {
          model: browserRoot.rows

          Item {
            required property int index
            required property var modelData

            width: parent.width
            height: Style.space(38)

            readonly property int seconds: Number(modelData.seconds || 0)
            readonly property int pct: Number(modelData.pct || 0)
            readonly property bool fromHistory: String(modelData.source || "") === "history"
            readonly property string titleText: String(modelData.title || "")
            readonly property string sourceText: fromHistory ? "history match" : "title match"
            readonly property string detailText: titleText.length > 0 && titleText !== String(modelData.label || "")
                                                 ? sourceText + "  " + titleText
                                                 : sourceText

            Text {
              id: browserName
              anchors.left: parent.left
              anchors.right: browserValue.left
              anchors.rightMargin: Style.space(10)
              anchors.top: parent.top
              text: String(modelData.label || "Page")
              color: index === 0 ? root.foreground : root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.bold: index === 0
              elide: Text.ElideRight
            }

            Text {
              id: browserValue
              width: Style.space(112)
              anchors.right: parent.right
              anchors.top: parent.top
              text: root.formatDuration(seconds) + "  " + pct + "%"
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.bodySmall
              font.bold: true
              horizontalAlignment: Text.AlignRight
              elide: Text.ElideRight
            }

            Text {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.top: browserName.bottom
              text: detailText
              color: root.faint
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              elide: Text.ElideRight
            }

            Rectangle {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.bottom: parent.bottom
              height: Style.space(5)
              radius: height / 2
              color: root.track

              Rectangle {
                width: parent.width * root.clamp01(seconds / browserRoot.maxSeconds)
                height: parent.height
                radius: parent.radius
                color: root.sliceColor(index + 2, index === 0 ? 0.86 : 0.62)
              }
            }
          }
        }
      }
    }
  }

  component EmptyState: Rectangle {
    property string text: ""
    property bool urgent: false

    width: parent ? parent.width : implicitWidth
    implicitHeight: Style.space(58)
    radius: Style.space(7)
    color: root.fill
    border.color: urgent ? Color.urgent : root.line
    border.width: 1

    Text {
      anchors.centerIn: parent
      width: parent.width - Style.space(20)
      text: parent.text
      color: parent.urgent ? Color.urgent : root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideRight
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
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

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
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

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

    function restartReveal() {
      revealProgress = 0
      monthRhythmReveal.restart()
    }

    onCellsChanged: restartReveal()
    onMaxSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: compact ? Style.space(398) : Style.space(304)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

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
  }

  component MonthHeatmap: Rectangle {
    id: monthRoot

    property string title: ""
    property string detail: ""
    property var cells: []
    property real maxSeconds: 0
    property int selectedIndex: -1
    property bool weekly: false
    property int hoveredIndex: -1
    property string hoveredText: ""
    property real revealProgress: 0
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < cells.length
      ? Model.monthCellDetailText(cells[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property string defaultText: Model.monthDefaultText(cells, weekly)

    readonly property bool cramped: width > 0 && width < Style.space(360)
    readonly property real gap: cramped ? Style.space(2) : Style.space(4)
    readonly property int columnCount: weekly ? 13 : 7
    readonly property var headerLabels: weekly ? Model.bucketLabels(columnCount) : Model.weekdayLabels()
    readonly property real maxCellSize: weekly ? (root.widePanel ? Style.space(44) : Style.space(38)) : (root.widePanel ? Style.space(54) : Style.space(38))
    readonly property real cellSize: Math.max(Style.space(8), Math.min(maxCellSize, (width - Style.space(24) - gap * (columnCount - 1)) / columnCount))
    readonly property int rowCount: Math.ceil(cells.length / columnCount)
    readonly property real gridWidth: columnCount * cellSize + Math.max(0, columnCount - 1) * gap

    function restartReveal() {
      revealProgress = 0
      monthReveal.restart()
    }

    onCellsChanged: restartReveal()
    onMaxSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: Style.space(108) + rowCount * cellSize + Math.max(0, rowCount - 1) * gap
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

    NumberAnimation {
      id: monthReveal

      target: monthRoot
      property: "revealProgress"
      from: 0
      to: 1
      duration: 620
      easing.type: Easing.OutCubic
    }

    Item {
      id: monthHeader

      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(12)
      height: Style.space(20)

      Text {
        anchors.left: parent.left
        anchors.right: monthDetail.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: monthRoot.title
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: monthDetail

        width: Math.min(implicitWidth, parent.width * 0.46)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: monthRoot.detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
      }
    }

    Row {
      id: weekdayHeader
      width: monthRoot.gridWidth
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.top: monthHeader.bottom
      anchors.topMargin: Style.space(8)
      spacing: monthRoot.gap

      Repeater {
        model: monthRoot.headerLabels

        Text {
          required property string modelData
          width: monthRoot.cellSize
          text: monthRoot.weekly ? modelData : modelData.substr(0, 1)
          color: root.faint
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          horizontalAlignment: Text.AlignHCenter
        }
      }
    }

    Grid {
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.top: weekdayHeader.bottom
      anchors.topMargin: Style.space(4)
      columns: monthRoot.columnCount
      rowSpacing: monthRoot.gap
      columnSpacing: monthRoot.gap

      Repeater {
        model: cells

        Rectangle {
          required property int index
          required property var modelData
          readonly property real cellSeconds: Number(modelData.seconds || 0)
          readonly property real cellIntensity: Model.heatIntensity(cellSeconds, monthRoot.maxSeconds)
          readonly property color heatBase: root.sliceColor(0, 1.0)

          width: monthRoot.cellSize
          height: width
          radius: Style.space(4)
          color: modelData.blank
            ? "transparent"
            : (cellSeconds > 0
              ? root.withAlpha(heatBase, 0.10 + monthRoot.revealProgress * (0.12 + 0.70 * cellIntensity))
              : root.track)
          border.color: modelData.blank ? "transparent" : root.line
          border.width: modelData.blank ? 0 : 1
          scale: (monthRoot.hoveredIndex === index || monthRoot.selectedIndex === index) && !modelData.blank ? 1.06 : 1.0

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
              monthRoot.hoveredIndex = index
              monthRoot.hoveredText = Model.monthCellDetailText(modelData)
            }
            onExited: {
              if (monthRoot.hoveredIndex === index) {
                monthRoot.hoveredIndex = -1
                monthRoot.hoveredText = ""
              }
            }
          }
        }
      }
    }

    ChartReadout {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.margins: Style.space(10)
      text: monthRoot.readoutText.length > 0 ? monthRoot.readoutText : monthRoot.defaultText
    }
  }

  component HeatmapGrid: Rectangle {
    id: heatRoot

    property string title: ""
    property string detail: ""
    property var cells: []
    property real maxSeconds: 0
    property int selectedIndex: -1
    property int hoveredIndex: -1
    property string hoveredText: ""
    property real revealProgress: 0
    property bool expanded: false
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < cells.length
      ? Model.heatCellDetailText(cells[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property string defaultText: Model.heatDefaultText(cells)

    readonly property bool cramped: width > 0 && width < Style.space(430)
    readonly property real labelWidth: cramped ? Style.space(22) : Style.space(30)
    readonly property real gap: cramped ? Style.space(1) : Style.space(2)
    readonly property real cellWidth: Math.max(Style.space(2), (width - Style.space(24) - labelWidth - gap * 23) / 24)
    readonly property real cellHeight: cramped ? Style.space(8) : Math.min(Style.space(16), Math.max(Style.space(10), cellWidth * 0.52))
    readonly property real gridHeight: 7 * cellHeight + 6 * gap

    function restartReveal() {
      revealProgress = 0
      heatReveal.restart()
    }

    onCellsChanged: restartReveal()
    onMaxSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: Math.max(expanded ? Style.space(216) : Style.space(184), Style.space(118) + gridHeight)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

    NumberAnimation {
      id: heatReveal

      target: heatRoot
      property: "revealProgress"
      from: 0
      to: 1
      duration: 700
      easing.type: Easing.OutCubic
    }

    Item {
      id: heatHeader

      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      anchors.margins: Style.space(12)
      height: Style.space(20)

      Text {
        anchors.left: parent.left
        anchors.right: heatDetail.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: heatRoot.title
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        id: heatDetail

        width: Math.min(implicitWidth, parent.width * 0.46)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: heatRoot.detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        horizontalAlignment: Text.AlignRight
        elide: Text.ElideRight
      }
    }

    Item {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: heatHeader.bottom
      anchors.bottom: parent.bottom
      anchors.leftMargin: Style.space(12)
      anchors.rightMargin: Style.space(12)
      anchors.topMargin: Style.space(8)
      anchors.bottomMargin: Style.space(12)

      Row {
        id: hourLabels
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: Style.space(14)

        Item {
          width: heatRoot.labelWidth
          height: 1
        }

        Item {
          width: parent.width - heatRoot.labelWidth
          height: parent.height

          Repeater {
            model: [0, 6, 12, 18, 23]

            Text {
              required property int modelData
              x: Math.min(parent.width - width, modelData * (heatRoot.cellWidth + heatRoot.gap))
              text: Model.hourLabel(modelData)
              color: root.faint
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
            }
          }
        }
      }

      Row {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: hourLabels.bottom
        anchors.topMargin: Style.space(4)
        spacing: 0

        Column {
          width: heatRoot.labelWidth
          spacing: heatRoot.gap

          Repeater {
            model: Model.weekdayLabels()

            Text {
              required property string modelData
              width: parent.width
              height: heatRoot.cellHeight
              text: modelData
              color: root.faint
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              verticalAlignment: Text.AlignVCenter
              elide: Text.ElideRight
            }
          }
        }

        Grid {
          id: heatCellsGrid
          columns: 24
          rowSpacing: heatRoot.gap
          columnSpacing: heatRoot.gap

          Repeater {
            model: cells

            Rectangle {
              required property int index
              required property var modelData
              readonly property real cellSeconds: Number(modelData.seconds || 0)
              readonly property real cellIntensity: Model.heatIntensity(cellSeconds, heatRoot.maxSeconds)
              readonly property color heatBase: root.sliceColor(2, 1.0)

              width: heatRoot.cellWidth
              height: heatRoot.cellHeight
              radius: Style.space(2)
              color: cellSeconds > 0
                ? root.withAlpha(heatBase, 0.08 + heatRoot.revealProgress * (0.12 + 0.76 * cellIntensity))
                : root.track
              border.color: heatRoot.hoveredIndex === index || heatRoot.selectedIndex === index ? root.foreground : "transparent"
              border.width: heatRoot.hoveredIndex === index || heatRoot.selectedIndex === index ? 1 : 0

              Behavior on color {
                ColorAnimation { duration: 140 }
              }

              Behavior on border.color {
                ColorAnimation { duration: 120 }
              }

              MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onEntered: {
                  heatRoot.hoveredIndex = index
                  heatRoot.hoveredText = Model.heatCellDetailText(modelData)
                }
                onExited: {
                  if (heatRoot.hoveredIndex === index) {
                    heatRoot.hoveredIndex = -1
                    heatRoot.hoveredText = ""
                  }
                }
              }
            }
          }
        }
      }

      Row {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Style.space(14)
        spacing: Style.space(5)

        Text {
          width: heatRoot.labelWidth
          text: "Low"
          color: root.faint
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }

        Repeater {
          model: 5

          Rectangle {
            required property int index

            width: Style.space(18)
            height: Style.space(8)
            radius: Style.space(2)
            anchors.verticalCenter: parent.verticalCenter
            color: Qt.rgba(root.sliceColor(2, 1.0).r, root.sliceColor(2, 1.0).g, root.sliceColor(2, 1.0).b, 0.18 + index * 0.17)
          }
        }

        Text {
          width: Style.space(92)
          text: Model.heatLegendHigh(heatRoot.maxSeconds)
          color: root.faint
          font.family: root.fontFamily
          font.pixelSize: Style.font.caption
          elide: Text.ElideRight
        }
      }
    }

    ChartReadout {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.leftMargin: Style.space(86)
      anchors.rightMargin: Style.space(10)
      anchors.bottomMargin: Style.space(26)
      text: heatRoot.readoutText.length > 0 ? heatRoot.readoutText : heatRoot.defaultText
    }
  }

  component ChartReadout: Rectangle {
    id: readoutRoot

    property string text: ""

    visible: text.length > 0
    implicitHeight: visible ? Style.space(24) : 0
    height: visible ? Style.space(24) : 0
    radius: Style.space(5)
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    border.color: root.line
    border.width: 1
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

  component InsightLanes: Column {
    id: lanesRoot

    property var groups: []

    spacing: Style.space(8)

    Repeater {
      model: groups

      Column {
        required property var modelData

        width: lanesRoot.width
        spacing: Style.space(6)

        Row {
          width: parent.width
          height: Style.space(18)
          spacing: Style.space(8)

          Text {
            width: implicitWidth
            anchors.verticalCenter: parent.verticalCenter
            text: String(modelData.title || "Insights")
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }

          Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: Style.space(4)
            height: width
            radius: width / 2
            color: root.faint
          }

          Text {
            anchors.verticalCenter: parent.verticalCenter
            text: String((modelData.rows || []).length) + " facts"
            color: root.faint
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }

        GridLayout {
          id: insightLaneGrid

          width: parent.width
          columns: root.widePanel ? 3 : (root.compactPanel ? 1 : 2)
          rowSpacing: Style.space(8)
          columnSpacing: Style.space(8)

          Repeater {
            model: (modelData.rows || []).slice(0, root.widePanel ? 6 : (root.compactPanel ? 3 : 4))

            InsightRow {
              required property var modelData

              Layout.fillWidth: true
              Layout.preferredWidth: insightLaneGrid.columns > 0
                ? Math.max(0, (insightLaneGrid.width - insightLaneGrid.columnSpacing * (insightLaneGrid.columns - 1)) / insightLaneGrid.columns)
                : insightLaneGrid.width
              title: String(modelData.label || "Insight")
              value: String(modelData.value || "")
              detail: String(modelData.detail || "")
              category: String(modelData.category || "")
              tone: String(modelData.tone || "")
            }
          }
        }
      }
    }
  }

  component InsightRow: Rectangle {
    property string title: ""
    property string value: ""
    property string detail: ""
    property string category: ""
    property string tone: ""

    implicitHeight: Style.space(88)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

    Rectangle {
      width: Style.space(3)
      height: parent.height - Style.space(16)
      radius: width / 2
      color: root.toneColor(tone)
      anchors.left: parent.left
      anchors.leftMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
    }

    Text {
      id: insightCategory

      anchors.right: parent.right
      anchors.rightMargin: Style.space(12)
      anchors.top: parent.top
      anchors.topMargin: Style.space(10)
      width: Math.min(implicitWidth, parent.width * 0.34)
      text: category.replace("-", " ")
      color: root.faint
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      horizontalAlignment: Text.AlignRight
      elide: Text.ElideRight
      visible: category.length > 0 && parent.width > Style.space(220)
    }

    Column {
      id: insightColumn
      anchors.left: parent.left
      anchors.leftMargin: Style.space(22)
      anchors.right: parent.right
      anchors.rightMargin: Style.space(12)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(3)

      Text {
        width: parent.width
        rightPadding: insightCategory.visible ? insightCategory.width + Style.space(8) : 0
        text: title
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: value
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        wrapMode: Text.WordWrap
        maximumLineCount: 2
        elide: Text.ElideRight
      }

      Text {
        visible: detail.length > 0
        width: parent.width
        text: detail
        color: root.faint
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        wrapMode: Text.WordWrap
        maximumLineCount: 1
        elide: Text.ElideRight
      }
    }
  }
}
