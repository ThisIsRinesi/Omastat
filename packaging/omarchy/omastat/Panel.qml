import QtQuick
import QtQuick.Layouts
import QtQuick.Window
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
  property var summaryTopApp: null
  property var reportInsights: []
  property var widgetInsight: null
  property var daily: []
  property var heatmap: []
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
  readonly property int metricColumns: panel.width > 0 && panel.width < Style.space(420) ? 1 : (compactPanel ? 2 : 5)
  readonly property bool appMixStacked: panel.width > 0 && panel.width < Style.space(680)
  readonly property bool usesCalendar: selectedLens === "month" || selectedLens === "year" || selectedLens === "life"
  readonly property bool usesWeeklyCalendar: selectedLens === "year" || selectedLens === "life"
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
  readonly property var sliceColors: Model.sliceColors(visibleApps.length, Color.accent)
  readonly property var segments: Model.arcSegments(visibleApps)
  readonly property int appLegendSplitIndex: Math.ceil(visibleApps.length / 2)
  readonly property var trendDays: Model.trendDays(daily, displayTodayKey, selectedLens)
  readonly property real trendMax: Model.maxDailySeconds(trendDays)
  readonly property string trendSummaryText: Model.weekTrendSummary(daily, displayTodayKey)
  readonly property var monthCells: Model.monthCells(daily, selectedLens)
  readonly property real monthMax: Model.maxMonthSeconds(monthCells)
  readonly property var heatCells: Model.heatmapCells(heatmap)
  readonly property real heatMax: Model.maxHeatSeconds(heatCells)
  readonly property var consistency: Model.consistencyStats(daily)
  readonly property var insightRows: reportInsights && reportInsights.length > 0 ? reportInsights : Model.insights(rows, daily, todayKey, totalFocused)
  readonly property bool hasFocusedData: totalFocused > 0 || visibleApps.length > 0
  readonly property bool showBreakdown: totalFocused + totalObserved + totalExcluded > 0
  readonly property real targetPanelWidth: Screen.width > 0 ? Math.min(Screen.width * 0.75, Style.space(1180)) : Style.space(1080)
  readonly property real targetPanelHeight: Screen.height > 0 ? Math.min(Screen.height * 0.88, Style.space(980)) : Style.space(820)
  readonly property bool widePanel: panel.width >= Style.space(900)
  readonly property bool showActivityChart: usesCalendar ? monthCells.length > 0 : trendDays.length > 0
  readonly property bool showHeatmapChart: heatMax > 0
  readonly property int analyticsColumns: widePanel && showActivityChart && showHeatmapChart ? 2 : 1
  readonly property string periodScopeLabel: selectedOffset === 0 && selectedLens !== "day" && selectedLens !== "life" ? periodLabel + " to date" : periodLabel
  readonly property string consistencyScopeText: selectedLens === "life" ? "Recent visible days" : (selectedOffset === 0 && selectedLens !== "day" ? "Elapsed days only" : "Across period")
  readonly property string loadingAppMixText: summaryTopApp && summaryTopApp.app ? "Loading app mix; top " + String(summaryTopApp.app) + " " + root.formatDuration(Number(summaryTopApp.seconds || 0)) : "Loading app mix..."

  onSelectedLensChanged: clearInspection()
  onSelectedOffsetChanged: clearInspection()
  onDailyChanged: inspectedActivityIndex = -1
  onHeatmapChanged: inspectedHeatIndex = -1

  function refresh() {
    if (hostWidget && hostWidget.refresh) hostWidget.refresh(true)
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
  }

  function inspectActivity(delta) {
    var count = root.usesCalendar ? root.monthCells.length : root.trendDays.length
    if (count <= 0) return

    var direction = delta < 0 ? -1 : 1
    var next = inspectedActivityIndex >= 0
      ? inspectedActivityIndex + direction
      : (direction > 0 ? 0 : count - 1)
    for (var i = 0; i < count; i++) {
      next = (next + count) % count
      if (!root.usesCalendar || !(root.monthCells[next] && root.monthCells[next].blank)) {
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
        else if (text === "h" || text === "H") root.inspectHeat(-1)
        else if (text === "l" || text === "L") root.inspectHeat(1)
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
              text: "Omastat"
              color: root.foreground
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
              elide: Text.ElideRight
            }

            Text {
              width: parent.width
              text: root.narrowPanel ? root.periodScopeLabel : root.periodScopeLabel + "  -  " + root.lensLabel
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

          GridLayout {
            width: parent.width
            columns: root.metricColumns
            rowSpacing: Style.space(8)
            columnSpacing: Style.space(8)

            MetricTile {
              Layout.fillWidth: true
              label: "Focused"
              value: root.formatDuration(root.totalFocused)
              detail: root.hasFocusedData ? "Focused time" : "No focus yet"
              accentColor: root.sliceColor(0, 1.0)
            }

            MetricTile {
              Layout.fillWidth: true
              label: "Focus Share"
              value: root.focusShareText
              detail: root.focusDenominator > 0 ? "Focused / observed" : "--"
              accentColor: root.sliceColor(1, 1.0)
            }

            MetricTile {
              Layout.fillWidth: true
              label: "Observed"
              value: root.totalObserved > 0 ? root.formatDuration(root.totalObserved) : "--"
              detail: root.observedDetailText
              accentColor: root.sliceColor(2, 1.0)
            }

            MetricTile {
              Layout.fillWidth: true
              label: "Paused"
              value: root.formatDuration(root.totalPaused)
              detail: "Away, lock, sleep"
              accentColor: root.sliceColor(3, 1.0)
            }

            MetricTile {
              Layout.fillWidth: true
              label: "Tracker Off"
              value: root.formatDuration(root.totalUnobserved)
              detail: root.totalUnobserved > 0 ? "Unobserved gap" : "No tracker gaps"
              accentColor: root.totalUnobserved > 0 ? Color.urgent : root.faint
            }
          }

          GridLayout {
            id: summaryGrid

            width: parent.width
            visible: root.showBreakdown || (root.widePanel && root.daily.length > 0)
            columns: root.widePanel && root.showBreakdown && root.daily.length > 0 ? 2 : 1
            rowSpacing: Style.space(12)
            columnSpacing: Style.space(12)

            Column {
              Layout.fillWidth: true
              Layout.preferredWidth: summaryGrid.columns > 1 ? Math.max(0, (summaryGrid.width - summaryGrid.columnSpacing) * 0.48) : summaryGrid.width
              Layout.alignment: Qt.AlignTop
              visible: root.showBreakdown
              spacing: Style.space(8)

              SectionHeader {
                text: "Tracked Signals"
              }

              TimeBreakdownStrip {
                width: parent.width
                focusedSeconds: root.totalFocused
                observedSeconds: root.totalObserved
                pausedSeconds: root.totalPaused
                trackerOffSeconds: root.totalUnobserved
                focusedShare: root.focusDenominator > 0 ? root.totalFocused / root.focusDenominator : 0
                excludedDetail: root.excludedDetailText
              }
            }

            Column {
              Layout.fillWidth: true
              Layout.preferredWidth: summaryGrid.columns > 1 ? Math.max(0, (summaryGrid.width - summaryGrid.columnSpacing) * 0.52) : summaryGrid.width
              Layout.alignment: Qt.AlignTop
              visible: root.widePanel && root.daily.length > 0
              spacing: Style.space(8)

              SectionHeader {
                text: root.selectedLens === "life" ? "Recent Consistency" : "Consistency"
              }

              ConsistencyMetrics {
                width: parent.width
                columnsValue: 4
              }
            }
          }

          SectionHeader {
            text: "App Mix by Focused Time"
          }

          Rectangle {
            id: appMixCard

            width: parent.width
            implicitHeight: appMixGrid.implicitHeight + Style.space(24)
            radius: Style.space(7)
            color: root.fill
            border.color: root.widePanel ? root.sliceColor(0, 0.32) : root.line
            border.width: 1
            visible: root.visibleApps.length > 0
            clip: true

            Rectangle {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.top: parent.top
              height: Style.space(2)
              color: root.sliceColor(0, 0.72)
              opacity: root.widePanel ? 1 : 0.72
            }

            GridLayout {
              id: appMixGrid

              readonly property bool splitLegend: root.widePanel && root.visibleApps.length >= 4

              anchors.left: parent.left
              anchors.right: parent.right
              anchors.top: parent.top
              anchors.margins: Style.space(12)
              columns: root.appMixStacked ? 1 : (splitLegend ? 3 : 2)
              rowSpacing: Style.space(12)
              columnSpacing: Style.space(18)

              DonutChart {
                id: appDonut
                Layout.preferredWidth: root.appMixStacked ? Style.space(150) : (appMixGrid.splitLegend ? Style.space(184) : (root.widePanel ? Style.space(220) : Style.space(168)))
                Layout.preferredHeight: Layout.preferredWidth
                Layout.alignment: Qt.AlignHCenter | Qt.AlignVCenter
                segments: root.segments
                colors: root.sliceColors
                centerLabel: "FOCUS"
                centerValue: root.formatDuration(root.totalFocused)
              }

              Column {
                id: appLegend

                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                visible: !appMixGrid.splitLegend
                spacing: Style.space(7)

                Repeater {
                  model: root.visibleApps

                  AppLegendRow {
                    required property int index
                    required property var modelData

                    width: parent.width
                    colorIndex: index
                    app: String(modelData.app || "")
                    category: String(modelData.category || "")
                    seconds: Number(modelData.seconds || 0)
                    pct: Number(modelData.pct || 0)
                  }
                }
              }

              Column {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                visible: appMixGrid.splitLegend
                spacing: Style.space(7)

                Repeater {
                  model: root.visibleApps.slice(0, root.appLegendSplitIndex)

                  AppLegendRow {
                    required property int index
                    required property var modelData

                    width: parent.width
                    colorIndex: index
                    app: String(modelData.app || "")
                    category: String(modelData.category || "")
                    seconds: Number(modelData.seconds || 0)
                    pct: Number(modelData.pct || 0)
                  }
                }
              }

              Column {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                visible: appMixGrid.splitLegend
                spacing: Style.space(7)

                Repeater {
                  model: root.visibleApps.slice(root.appLegendSplitIndex)

                  AppLegendRow {
                    required property int index
                    required property var modelData

                    width: parent.width
                    colorIndex: root.appLegendSplitIndex + index
                    app: String(modelData.app || "")
                    category: String(modelData.category || "")
                    seconds: Number(modelData.seconds || 0)
                    pct: Number(modelData.pct || 0)
                  }
                }
              }
            }
          }

          EmptyState {
            visible: root.visibleApps.length === 0
            text: root.errorText !== "" ? root.errorText : (root.refreshRunning && root.totalFocused > 0 ? root.loadingAppMixText : "No focused app time for this period")
            urgent: root.errorText !== ""
          }

          GridLayout {
            id: analyticsGrid

            width: parent.width
            visible: root.showActivityChart || root.showHeatmapChart
            columns: root.analyticsColumns
            rowSpacing: Style.space(12)
            columnSpacing: Style.space(12)

            Column {
              Layout.fillWidth: true
              Layout.preferredWidth: root.analyticsColumns > 1 ? Math.max(0, (analyticsGrid.width - analyticsGrid.columnSpacing) / 2) : analyticsGrid.width
              Layout.alignment: Qt.AlignTop
              visible: root.showActivityChart
              spacing: Style.space(8)

              SectionHeader {
                text: root.selectedLens === "life" ? "Recent Weekly Activity" : (root.usesWeeklyCalendar ? "Weekly Activity" : (root.usesCalendar ? "Activity Calendar" : "Daily Focus"))
              }

              TrendBars {
                width: parent.width
                expanded: root.widePanel
                visible: !root.usesCalendar && root.trendDays.length > 0
                days: root.trendDays
                maxSeconds: root.trendMax
                selectedIndex: root.inspectedActivityIndex
              }

              MonthHeatmap {
                width: parent.width
                visible: root.usesCalendar && root.monthCells.length > 0
                cells: root.monthCells
                maxSeconds: root.monthMax
                selectedIndex: root.inspectedActivityIndex
                weekly: root.usesWeeklyCalendar
              }

              Text {
                visible: root.trendSummaryText !== ""
                width: parent.width
                text: root.trendSummaryText
                color: root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                elide: Text.ElideRight
              }
            }

            Column {
              Layout.fillWidth: true
              Layout.preferredWidth: root.analyticsColumns > 1 ? Math.max(0, (analyticsGrid.width - analyticsGrid.columnSpacing) / 2) : analyticsGrid.width
              Layout.alignment: Qt.AlignTop
              visible: root.showHeatmapChart
              spacing: Style.space(8)

              SectionHeader {
                text: root.selectedLens === "day" || root.selectedLens === "week" ? "Focus by Time of Week" : "Cumulative Focus by Time of Week"
              }

              HeatmapGrid {
                width: parent.width
                expanded: root.widePanel
                cells: root.heatCells
                maxSeconds: root.heatMax
                selectedIndex: root.inspectedHeatIndex
              }
            }
          }

          SectionHeader {
            text: root.selectedLens === "life" ? "Recent Consistency" : "Consistency"
            visible: !root.widePanel && root.daily.length > 0
          }

          ConsistencyMetrics {
            width: parent.width
            visible: !root.widePanel && root.daily.length > 0
            columnsValue: root.tileColumns
          }

          SectionHeader {
            text: "Insights"
            visible: root.insightRows.length > 0
          }

          Column {
            width: parent.width
            visible: root.insightRows.length > 0
            spacing: Style.space(8)

            Repeater {
              model: root.insightRows.slice(0, 5)

              InsightRow {
                required property var modelData

                width: parent.width
                title: String(modelData.label || "Insight")
                value: String(modelData.value || "")
                detail: String(modelData.detail || "")
                tone: String(modelData.tone || "")
              }
            }
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

    readonly property int maxSeconds: Math.max(1, focusedSeconds, observedSeconds, pausedSeconds, trackerOffSeconds)

    function restartReveal() {
      revealProgress = 0
      breakdownReveal.restart()
    }

    onFocusedSecondsChanged: restartReveal()
    onObservedSecondsChanged: restartReveal()
    onPausedSecondsChanged: restartReveal()
    onTrackerOffSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: Style.space(178)
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

      BreakdownBar {
        width: parent.width
        label: "Focused Time"
        value: root.formatDuration(focusedSeconds) + (observedSeconds > 0 ? "  " + Model.percent(focusedShare) : "")
        ratio: focusedSeconds / breakdownRoot.maxSeconds
        revealProgress: breakdownRoot.revealProgress
        colorValue: root.sliceColor(0, 0.95)
        detail: observedSeconds > 0 ? "Share of observed time" : ""
      }

      BreakdownBar {
        width: parent.width
        label: "Observed Time"
        value: observedSeconds > 0 ? root.formatDuration(observedSeconds) : "--"
        ratio: observedSeconds / breakdownRoot.maxSeconds
        revealProgress: breakdownRoot.revealProgress
        colorValue: root.sliceColor(1, 0.78)
        detail: root.observedDetailText
      }

      BreakdownBar {
        width: parent.width
        label: "Paused"
        value: root.formatDuration(pausedSeconds)
        ratio: pausedSeconds / breakdownRoot.maxSeconds
        revealProgress: breakdownRoot.revealProgress
        colorValue: root.sliceColor(3, 1.0)
        detail: root.pausedDetailText.length > 0 ? root.pausedDetailText : "Away, lock, sleep"
      }

      BreakdownBar {
        width: parent.width
        label: "Tracker Off"
        value: root.formatDuration(trackerOffSeconds)
        ratio: trackerOffSeconds / breakdownRoot.maxSeconds
        revealProgress: breakdownRoot.revealProgress
        colorValue: trackerOffSeconds > 0 ? Color.urgent : root.faint
        detail: trackerOffSeconds > 0 ? "Unobserved time excluded from focus share" : "No tracker gaps"
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

  component DonutChart: Item {
    id: donutRoot

    property var segments: []
    property var colors: []
    property string centerLabel: ""
    property string centerValue: ""
    property real revealProgress: 0

    implicitWidth: width
    implicitHeight: height
    opacity: segments.length > 0 ? 1 : 0.55

    Behavior on opacity {
      NumberAnimation { duration: 180 }
    }

    function restartReveal() {
      revealProgress = 0
      donutReveal.restart()
    }

    onSegmentsChanged: {
      restartReveal()
      donutCanvas.requestPaint()
    }
    onColorsChanged: donutCanvas.requestPaint()
    onRevealProgressChanged: donutCanvas.requestPaint()
    onWidthChanged: donutCanvas.requestPaint()
    onHeightChanged: donutCanvas.requestPaint()
    Component.onCompleted: restartReveal()

    NumberAnimation {
      id: donutReveal

      target: donutRoot
      property: "revealProgress"
      from: 0
      to: 1
      duration: 680
      easing.type: Easing.OutCubic
    }

    Canvas {
      id: donutCanvas
      anchors.fill: parent

      onPaint: {
        var ctx = getContext("2d")
        ctx.reset()
        var segs = segments || []
        var size = Math.min(width, height)
        var line = Math.max(Style.space(10), Math.round(size * 0.12))
        var radius = size / 2 - line / 2
        var cx = width / 2
        var cy = height / 2
        var progress = root.clamp01(donutRoot.revealProgress)
        var remainingSweep = 360 * progress

        ctx.lineCap = "round"
        ctx.lineWidth = line + Style.space(2)
        ctx.strokeStyle = root.withAlpha(root.foreground, 0.07)
        ctx.beginPath()
        ctx.arc(cx, cy, radius, 0, Math.PI * 2, false)
        ctx.stroke()

        ctx.lineWidth = line
        ctx.strokeStyle = root.track
        ctx.beginPath()
        ctx.arc(cx, cy, radius, 0, Math.PI * 2, false)
        ctx.stroke()

        for (var i = 0; i < segs.length; i++) {
          var seg = segs[i]
          var drawSweep = Math.min(Number(seg.sweepAngle || 0), Math.max(0, remainingSweep))
          remainingSweep -= Number(seg.sweepAngle || 0)
          if (drawSweep <= 0) continue

          ctx.strokeStyle = root.colorFromHex(String(donutRoot.colors[i] || Color.accent), 1.0)
          ctx.beginPath()
          ctx.arc(cx, cy, radius, seg.startAngle * Math.PI / 180, (seg.startAngle + drawSweep) * Math.PI / 180, false)
          ctx.stroke()
        }

        ctx.lineCap = "butt"
        ctx.lineWidth = Math.max(1, Style.space(1))
        ctx.strokeStyle = root.withAlpha(root.foreground, 0.12)
        ctx.beginPath()
        ctx.arc(cx, cy, Math.max(1, radius - line / 2 - Style.space(5)), 0, Math.PI * 2, false)
        ctx.stroke()
      }
    }

    Column {
      anchors.centerIn: parent
      width: parent.width * 0.62
      spacing: Style.space(1)

      Text {
        width: parent.width
        text: centerLabel
        color: root.dim
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
      }

      Text {
        width: parent.width
        text: centerValue
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        font.bold: true
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
      }
    }
  }

  component AppLegendRow: Item {
    property int colorIndex: 0
    property string app: ""
    property string category: ""
    property int seconds: 0
    property int pct: 0

    implicitHeight: Math.max(Style.space(26), Math.max(appName.implicitHeight, appTime.implicitHeight) + Style.space(8))

    Rectangle {
      id: swatch
      width: Style.space(8)
      height: width
      radius: width / 2
      color: root.sliceColor(colorIndex, 1.0)
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
    }

    Text {
      id: appName
      text: app
      color: root.foreground
      opacity: 0.78
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
      anchors.left: swatch.right
      anchors.leftMargin: Style.space(7)
      anchors.right: categoryText.left
      anchors.rightMargin: Style.space(8)
      anchors.verticalCenter: parent.verticalCenter
    }

    Text {
      id: categoryText
      visible: category !== "" && category !== "neutral" && category !== "mixed"
      width: visible ? Math.min(implicitWidth, Style.space(82)) : 0
      text: category
      color: root.faint
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      elide: Text.ElideRight
      anchors.right: appTime.left
      anchors.rightMargin: visible ? Style.space(8) : 0
      anchors.verticalCenter: parent.verticalCenter
    }

    Text {
      id: appTime
      width: Math.min(implicitWidth, Style.space(104))
      text: root.formatDuration(seconds) + "  " + pct + "%"
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      font.bold: true
      horizontalAlignment: Text.AlignRight
      elide: Text.ElideRight
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
    }

    Rectangle {
      id: pctTrack

      anchors.left: swatch.right
      anchors.leftMargin: Style.space(7)
      anchors.right: appTime.left
      anchors.rightMargin: Style.space(10)
      anchors.bottom: parent.bottom
      height: Style.space(2)
      radius: height / 2
      color: root.track
      visible: width > Style.space(18)

      Rectangle {
        width: pctTrack.width * root.clamp01(pct / 100)
        height: parent.height
        radius: parent.radius
        color: root.sliceColor(colorIndex, 0.72)

        Behavior on width {
          NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
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

  component TrendBars: Rectangle {
    id: trendRoot

    property var days: []
    property real maxSeconds: 0
    property int selectedIndex: -1
    property string hoveredKey: ""
    property string hoveredText: ""
    property real revealProgress: 0
    property bool expanded: false
    readonly property string selectedText: selectedIndex >= 0 && selectedIndex < days.length
      ? Model.trendDetailText(days[selectedIndex])
      : ""
    readonly property string readoutText: hoveredText.length > 0 ? hoveredText : selectedText
    readonly property string defaultText: Model.trendDefaultText(days)

    function restartReveal() {
      revealProgress = 0
      trendReveal.restart()
    }

    onDaysChanged: restartReveal()
    onMaxSecondsChanged: restartReveal()
    Component.onCompleted: restartReveal()

    implicitHeight: expanded ? Style.space(198) : Style.space(164)
    radius: Style.space(7)
    color: root.fill
    border.color: root.line
    border.width: 1

    NumberAnimation {
      id: trendReveal

      target: trendRoot
      property: "revealProgress"
      from: 0
      to: 1
      duration: 560
      easing.type: Easing.OutCubic
    }

    Row {
      anchors.fill: parent
      anchors.margins: Style.space(12)
      anchors.bottomMargin: Style.space(38)
      spacing: Style.space(7)

      Repeater {
        model: days

        Item {
          required property int index
          required property var modelData

          readonly property bool active: trendRoot.hoveredKey === String(modelData.key || "") || trendRoot.selectedIndex === index

          width: days.length > 0 ? (parent.width - parent.spacing * (days.length - 1)) / days.length : 0
          height: parent.height

          Column {
            anchors.fill: parent
            spacing: Style.space(3)

            Text {
              width: parent.width
              text: String(modelData.valueText || "")
              color: modelData.isToday ? root.foreground : root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              font.bold: modelData.isToday
              horizontalAlignment: Text.AlignHCenter
              elide: Text.ElideRight
            }

            Item {
              id: barSlot
              width: parent.width
              height: Math.max(Style.space(62), trendRoot.height - Style.space(102))

              Repeater {
                model: 3

                Rectangle {
                  required property int index

                  width: parent.width
                  height: 1
                  y: Math.round((index + 1) * barSlot.height / 4)
                  color: root.line
                  opacity: 0.36
                }
              }

              Rectangle {
                width: Math.max(Style.space(7), parent.width * 0.48)
                height: parent.height
                radius: Style.space(3)
                color: root.track
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
              }

              Rectangle {
                visible: Number(modelData.seconds || 0) > 0 && maxSeconds > 0
                width: Math.max(Style.space(7), parent.width * 0.48)
                radius: Style.space(3)
                color: active || modelData.isToday ? root.sliceColor(0, 1.0) : root.sliceColor(0, 0.48)
                border.color: active ? root.withAlpha(root.foreground, 0.42) : "transparent"
                border.width: active ? 1 : 0
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                height: Math.max(Style.space(4), barSlot.height * Number(modelData.seconds || 0) / maxSeconds * trendRoot.revealProgress)

                Behavior on height {
                  NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
                }

                Behavior on color {
                  ColorAnimation { duration: 140 }
                }

                Behavior on border.color {
                  ColorAnimation { duration: 140 }
                }
              }
            }

            Text {
              width: parent.width
              text: String(modelData.label || "")
              color: modelData.isToday ? root.foreground : root.faint
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              horizontalAlignment: Text.AlignHCenter
              elide: Text.ElideRight
            }

            Text {
              width: parent.width
              text: String(modelData.densityText || "--")
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
            cursorShape: Qt.PointingHandCursor
            onEntered: {
              trendRoot.hoveredKey = String(modelData.key || "")
              trendRoot.hoveredText = Model.trendDetailText(modelData)
            }
            onExited: {
              if (trendRoot.hoveredKey === String(modelData.key || "")) {
                trendRoot.hoveredKey = ""
                trendRoot.hoveredText = ""
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
      text: trendRoot.readoutText.length > 0 ? trendRoot.readoutText : trendRoot.defaultText
    }
  }

  component MonthHeatmap: Rectangle {
    id: monthRoot

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

    implicitHeight: Style.space(80) + rowCount * cellSize + Math.max(0, rowCount - 1) * gap
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

    Row {
      id: weekdayHeader
      width: monthRoot.gridWidth
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.top: parent.top
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

    implicitHeight: Math.max(expanded ? Style.space(198) : Style.space(166), Style.space(92) + gridHeight)
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
      anchors.fill: parent
      anchors.margins: Style.space(12)

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
    implicitHeight: visible ? readoutLabel.implicitHeight + Style.space(6) : 0
    height: implicitHeight
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

  component InsightRow: Rectangle {
    property string title: ""
    property string value: ""
    property string detail: ""
    property string tone: ""

    implicitHeight: insightColumn.implicitHeight + Style.space(18)
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

    Column {
      id: insightColumn
      anchors.left: parent.left
      anchors.leftMargin: Style.space(22)
      anchors.right: parent.right
      anchors.rightMargin: Style.space(12)
      anchors.verticalCenter: parent.verticalCenter
      spacing: Style.space(2)

      Text {
        width: parent.width
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
        maximumLineCount: 2
        elide: Text.ElideRight
      }
    }
  }
}
