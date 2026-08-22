import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui
import "Model.js" as Model

Panel {
  id: root
  moduleName: "local.omastat"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  property var rows: []
  property var reportApps: []
  property var reportInsights: []
  property var widgetInsight: null
  property var daily: []
  property string todayKey: ""
  property string periodLabel: "Today"
  property int totalFocused: 0
  property int totalOpen: 0
  property string statusText: ""
  property string errorText: ""
  property string updatedText: ""
  property bool patternsExpanded: false

  readonly property var barIdentity: hostWidget || root
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property color accent: bar ? bar.urgent : Color.accent
  readonly property color dim: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.58)
  readonly property color track: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.12)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property var apps: reportApps && reportApps.length > 0 ? reportApps : Model.groupedApps(Model.appList(rows), Model.DONUT_MAX_SLICES)
  readonly property var segments: Model.arcSegments(apps)
  readonly property var sliceColors: Model.sliceColors(apps.length, Color.accent)
  readonly property var insightRows: reportInsights && reportInsights.length > 0 ? reportInsights : Model.insights(rows, daily, todayKey, totalFocused)
  readonly property var weekTrend: Model.weekTrend(daily, todayKey)
  readonly property string trendSummaryText: Model.weekTrendSummary(daily, todayKey)
  readonly property bool hasTrendDetails: weekTrend.length > 0 || insightRows.length > 0
  readonly property string densityText: totalOpen > 0 ? Model.percent(totalFocused / totalOpen) : "--"
  readonly property real weekMax: {
    var max = 0
    for (var i = 0; i < weekTrend.length; i++) max = Math.max(max, Number(weekTrend[i].seconds || 0))
    return max
  }
  readonly property real ringSize: Style.space(116)
  readonly property real ringWidth: Style.space(14)
  readonly property real ringRadius: ringSize / 2 - ringWidth / 2

  function refresh() {
    if (hostWidget && hostWidget.refresh) hostWidget.refresh()
  }

  function openOverviewReport() {
    if (hostWidget && hostWidget.openOverviewReport) hostWidget.openOverviewReport()
  }

  function formatDuration(seconds) {
    return Model.fmt(seconds)
  }

  function sliceColor(index, alpha) {
    var hex = String(root.sliceColors[index] || Color.accent).replace(/[#\s]/g, "")
    var r = parseInt(hex.substr(0, 2), 16) / 255
    var g = parseInt(hex.substr(2, 2), 16) / 255
    var b = parseInt(hex.substr(4, 2), 16) / 255
    if (isNaN(r) || isNaN(g) || isNaN(b)) return Qt.rgba(root.accent.r, root.accent.g, root.accent.b, alpha)
    return Qt.rgba(r, g, b, alpha)
  }

  function insightToneColor(tone) {
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

  function togglePatterns() {
    if (!root.hasTrendDetails) return
    root.patternsExpanded = !root.patternsExpanded
  }

  onHasTrendDetailsChanged: if (!hasTrendDetails) patternsExpanded = false

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(390))
    contentHeight: panel.fittedContentHeight(headerRow.implicitHeight + Style.space(12) + contentColumn.implicitHeight, Style.space(500))

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        if (dy !== 0) root.scrollBy(-dy * Style.space(24))
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(text) {
        if (text === "r" || text === "R") root.refresh()
        else if (text === "o" || text === "O") root.openOverviewReport()
        else if (text === "p" || text === "P") root.togglePatterns()
      }

      Item {
        id: headerRow
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        implicitHeight: Math.max(titleColumn.implicitHeight, patternsToggle.implicitHeight)

        Column {
          id: titleColumn
          anchors.left: parent.left
          anchors.right: patternsToggle.left
          anchors.rightMargin: Style.space(10)
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(2)

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
            text: root.periodLabel || "Today"
            color: root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            elide: Text.ElideRight
          }
        }

        Row {
          id: patternsToggle
          visible: root.hasTrendDetails
          width: visible ? implicitWidth : 0
          anchors.right: parent.right
          anchors.top: parent.top
          spacing: Style.space(3)

          Text {
            text: "PATTERNS"
            color: patternsMouse.containsMouse ? root.foreground : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
          }

          Text {
            text: root.patternsExpanded ? "v" : ">"
            color: patternsMouse.containsMouse ? root.foreground : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
          }
        }

        MouseArea {
          id: patternsMouse
          anchors.fill: patternsToggle
          enabled: root.hasTrendDetails
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.togglePatterns()
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

          RowLayout {
            width: parent.width
            spacing: Style.space(14)

            StatBlock {
              Layout.fillWidth: true
              label: "Open time"
              value: root.formatDuration(root.totalOpen)
            }

            StatBlock {
              Layout.fillWidth: true
              label: "Focus share"
              value: root.densityText
            }

            StatBlock {
              Layout.fillWidth: true
              label: "Updated"
              value: root.updatedText || "--"
            }
          }

          PanelSeparator {
            foreground: root.foreground
          }

          Item {
            width: parent.width
            visible: root.apps.length > 0
            implicitHeight: visible ? Math.max(root.ringSize, legendColumn.implicitHeight) : 0

            Item {
              id: donutItem
              width: root.ringSize
              height: root.ringSize
              anchors.left: parent.left
              anchors.verticalCenter: parent.verticalCenter

              Canvas {
                id: donutCanvas
                anchors.fill: parent

                Connections {
                  target: root
                  function onSegmentsChanged() { donutCanvas.requestPaint() }
                  function onSliceColorsChanged() { donutCanvas.requestPaint() }
                }

                onPaint: {
                  var ctx = getContext("2d")
                  ctx.reset()
                  var segs = root.segments
                  if (!segs || segs.length === 0) return
                  var toRad = Math.PI / 180
                  for (var i = 0; i < segs.length; i++) {
                    var seg = segs[i]
                    ctx.lineWidth = root.ringWidth
                    ctx.strokeStyle = root.sliceColor(i, 1.0)
                    ctx.beginPath()
                    ctx.arc(width / 2, height / 2, root.ringRadius, seg.startAngle * toRad, (seg.startAngle + seg.sweepAngle) * toRad, false)
                    ctx.stroke()
                  }
                }
              }

              Column {
                anchors.centerIn: parent
                width: parent.width * 0.62
                spacing: Style.space(1)

                Text {
                  width: parent.width
                  text: "FOCUSED"
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  font.bold: true
                  horizontalAlignment: Text.AlignHCenter
                  elide: Text.ElideRight
                }

                Text {
                  width: parent.width
                  text: root.formatDuration(root.totalFocused)
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  font.bold: true
                  horizontalAlignment: Text.AlignHCenter
                  elide: Text.ElideRight
                }
              }
            }

            Column {
              id: legendColumn
              anchors.left: donutItem.right
              anchors.leftMargin: Style.space(16)
              anchors.right: parent.right
              anchors.verticalCenter: parent.verticalCenter
              spacing: Style.space(5)

              Repeater {
                model: root.apps

                Item {
                  required property var modelData
                  required property int index

                  width: parent.width
                  implicitHeight: Math.max(swatch.height, Math.max(appNameText.implicitHeight, appTimeText.implicitHeight))

                  Rectangle {
                    id: swatch
                    width: Style.space(7)
                    height: width
                    radius: width / 2
                    color: root.sliceColors[index] || root.accent
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                  }

                  Text {
                    id: appNameText
                    text: String(modelData.app || "")
                    color: root.foreground
                    opacity: 0.68
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.bodySmall
                    elide: Text.ElideRight
                    anchors.left: swatch.right
                    anchors.leftMargin: Style.space(6)
                    anchors.right: appTimeText.left
                    anchors.rightMargin: Style.space(8)
                    anchors.verticalCenter: parent.verticalCenter
                  }

                  Text {
                    id: appTimeText
                    text: Model.fmt(Number(modelData.seconds || 0))
                    color: root.foreground
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.bodySmall
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                  }
                }
              }
            }
          }

          Text {
            visible: root.apps.length === 0
            width: parent.width
            text: root.errorText !== "" ? root.errorText : "No focused app time today"
            color: root.errorText !== "" ? Color.urgent : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.body
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
          }

          Item {
            width: parent.width
            visible: root.patternsExpanded && root.hasTrendDetails
            implicitHeight: visible ? patternsColumn.implicitHeight : 0

            Column {
              id: patternsColumn
              width: parent.width
              spacing: Style.space(10)

              PanelSeparator {
                width: parent.width
                foreground: root.foreground
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

              Item {
                width: parent.width
                visible: root.weekTrend.length > 0
                implicitHeight: visible ? Style.space(82) : 0

                Row {
                  anchors.fill: parent
                  spacing: Style.space(6)

                  Repeater {
                    model: root.weekTrend

                    Item {
                      required property var modelData

                      width: root.weekTrend.length > 0 ? (parent.width - parent.spacing * (root.weekTrend.length - 1)) / root.weekTrend.length : 0
                      height: parent.height

                      Column {
                        anchors.fill: parent
                        spacing: Style.space(2)

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
                          id: trendSlot
                          width: parent.width
                          height: Style.space(42)

                          Rectangle {
                            width: Math.max(Style.space(6), parent.width * 0.46)
                            height: parent.height
                            radius: Style.space(2)
                            color: root.track
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.bottom: parent.bottom
                          }

                          Rectangle {
                            visible: Number(modelData.seconds || 0) > 0 && root.weekMax > 0
                            width: Math.max(Style.space(6), parent.width * 0.46)
                            radius: Style.space(2)
                            color: modelData.isToday ? root.sliceColor(0, 1.0) : root.sliceColor(0, 0.38)
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.bottom: parent.bottom
                            height: {
                              if (modelData.seconds <= 0 || root.weekMax <= 0) return 0
                              return Math.max(3, trendSlot.height * Number(modelData.seconds) / root.weekMax)
                            }
                          }
                        }

                        Text {
                          width: parent.width
                          text: String(modelData.label || "")
                          color: root.foreground
                          opacity: modelData.isToday ? 1.0 : 0.55
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          horizontalAlignment: Text.AlignHCenter
                          elide: Text.ElideRight
                        }

                        Text {
                          width: parent.width
                          text: String(modelData.focusShareText || modelData.densityText || "--")
                          color: root.dim
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          horizontalAlignment: Text.AlignHCenter
                          elide: Text.ElideRight
                        }
                      }
                    }
                  }
                }
              }

              PanelSeparator {
                width: parent.width
                foreground: root.foreground
              }

              Repeater {
                model: root.insightRows

                Item {
                  required property var modelData

                  width: parent.width
                  implicitHeight: insightColumn.implicitHeight

                  Rectangle {
                    width: Style.space(3)
                    height: parent.height
                    radius: width / 2
                    color: root.insightToneColor(modelData.tone)
                    opacity: 0.75
                    anchors.left: parent.left
                    anchors.top: parent.top
                  }

                  Column {
                    id: insightColumn
                    anchors.left: parent.left
                    anchors.leftMargin: Style.space(9)
                    anchors.right: parent.right
                    spacing: Style.space(2)

                    Item {
                      width: parent.width
                      implicitHeight: Math.max(labelText.implicitHeight, valueText.implicitHeight)

                      Text {
                        id: labelText
                        text: String(modelData.label || "")
                        color: root.dim
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.bodySmall
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Text.ElideRight
                        width: parent.width * 0.42
                      }

                      Text {
                        id: valueText
                        text: String(modelData.value || "")
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.bodySmall
                        font.bold: true
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        horizontalAlignment: Text.AlignRight
                        elide: Text.ElideRight
                        width: parent.width * 0.56
                      }
                    }

                    Text {
                      visible: String(modelData.detail || "").length > 0
                      width: parent.width
                      text: String(modelData.detail || "")
                      color: root.dim
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                      wrapMode: Text.WordWrap
                      maximumLineCount: 2
                      elide: Text.ElideRight
                    }
                  }
                }
              }
            }
          }

          PanelSeparator {
            foreground: root.foreground
          }

          Text {
            visible: root.errorText !== "" || root.statusText === "Refreshing"
            width: parent.width
            text: root.errorText !== "" ? root.errorText : "Refreshing..."
            color: root.errorText !== "" ? Color.urgent : root.dim
            font.family: root.fontFamily
            font.pixelSize: Style.font.bodySmall
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
          }

          RowLayout {
            width: parent.width
            spacing: Style.space(8)

            Button {
              Layout.fillWidth: true
              text: "Refresh"
              foreground: root.foreground
              accent: root.accent
              onClicked: root.refresh()
            }

            Button {
              Layout.fillWidth: true
              text: "Overview"
              foreground: root.foreground
              accent: root.accent
              onClicked: root.openOverviewReport()
            }
          }
        }
      }
    }
  }

  component StatBlock: ColumnLayout {
    property string label: ""
    property string value: ""

    spacing: Style.space(2)

    Text {
      Layout.fillWidth: true
      text: label
      color: root.dim
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
    }

    Text {
      Layout.fillWidth: true
      text: value
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      font.bold: true
      elide: Text.ElideRight
    }
  }
}
