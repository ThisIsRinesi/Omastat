import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui

Panel {
  id: root
  moduleName: "local.omastat"
  manageIpc: false

  property var anchorItem: null
  property var hostWidget: null
  property var rows: []
  property int totalFocused: 0
  property int totalOpen: 0
  property string statusText: ""
  property string errorText: ""
  property string updatedText: ""

  readonly property var barIdentity: hostWidget || root
  readonly property color foreground: bar ? bar.barForeground : Color.foreground
  readonly property color accent: bar ? bar.urgent : Color.accent
  readonly property color dim: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.58)
  readonly property color track: Qt.rgba(foreground.r, foreground.g, foreground.b, 0.12)
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family
  readonly property var topRows: rows.slice(0, Math.min(rows.length, 8))

  function refresh() {
    if (hostWidget && hostWidget.refresh) hostWidget.refresh()
  }

  function openTerminalReport() {
    if (hostWidget && hostWidget.openTerminalReport) hostWidget.openTerminalReport()
  }

  function rowShare(row) {
    if (totalFocused <= 0) return 0
    return Math.max(0, Math.min(1, Number(row.focused_seconds || 0) / totalFocused))
  }

  function appLabel(app) {
    var value = String(app || "App").replace(/^com\./, "").replace(/^org\./, "")
    var parts = value.split(".")
    return parts[parts.length - 1] || value
  }

  function formatDuration(seconds) {
    seconds = Math.max(0, Math.floor(seconds))
    if (seconds < 60) return seconds + "s"
    var minutes = Math.floor(seconds / 60)
    var hours = Math.floor(minutes / 60)
    if (hours > 0) return hours + "h " + String(minutes % 60).padStart(2, "0") + "m"
    return minutes + "m"
  }

  KeyboardPanel {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(Style.space(420))
    contentHeight: panel.fittedContentHeight(contentColumn.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(text) {
        if (text === "r" || text === "R") root.refresh()
        else if (text === "t" || text === "T") root.openTerminalReport()
      }

      Flickable {
        id: scroll
        anchors.fill: parent
        contentWidth: contentColumn.width
        contentHeight: contentColumn.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: contentHeight > height

        Column {
          id: contentColumn
          width: scroll.width
          spacing: Style.space(10)

          RowLayout {
            width: parent.width
            spacing: Style.space(12)

            ColumnLayout {
              Layout.fillWidth: true
              spacing: Style.space(2)

              Text {
                Layout.fillWidth: true
                text: "Omastat"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.title
                font.bold: true
                elide: Text.ElideRight
              }

              Text {
                Layout.fillWidth: true
                text: root.errorText !== "" ? root.errorText : root.statusText
                color: root.errorText !== "" ? Color.urgent : root.dim
                font.family: root.fontFamily
                font.pixelSize: Style.font.bodySmall
                elide: Text.ElideRight
              }
            }

            Text {
              text: root.formatDuration(root.totalFocused)
              color: root.accent
              font.family: root.fontFamily
              font.pixelSize: Style.font.title
              font.bold: true
            }
          }

          RowLayout {
            width: parent.width
            spacing: Style.space(14)

            StatBlock {
              Layout.fillWidth: true
              label: "Focused"
              value: root.formatDuration(root.totalFocused)
            }

            StatBlock {
              Layout.fillWidth: true
              label: "Open"
              value: root.formatDuration(root.totalOpen)
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

          Column {
            id: appList
            width: parent.width
            spacing: Style.space(7)

            Text {
              visible: root.topRows.length === 0
              width: parent.width
              text: "No focused app time today"
              color: root.dim
              font.family: root.fontFamily
              font.pixelSize: Style.font.body
              horizontalAlignment: Text.AlignHCenter
            }

            Repeater {
              model: root.topRows

              Item {
                required property var modelData
                required property int index

                width: appList.width
                height: Math.max(nameText.implicitHeight, Style.space(20))

                Text {
                  id: rankText
                  width: Style.space(24)
                  anchors.left: parent.left
                  anchors.verticalCenter: parent.verticalCenter
                  text: String(index + 1)
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  horizontalAlignment: Text.AlignRight
                }

                Text {
                  id: nameText
                  anchors.left: rankText.right
                  anchors.leftMargin: Style.space(8)
                  anchors.right: durationText.left
                  anchors.rightMargin: Style.space(12)
                  anchors.verticalCenter: parent.verticalCenter
                  text: root.appLabel(modelData.app_class)
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  elide: Text.ElideRight
                }

                Rectangle {
                  anchors.left: nameText.left
                  anchors.right: nameText.right
                  anchors.bottom: parent.bottom
                  height: Math.max(1, Style.space(2))
                  radius: height / 2
                  color: root.track

                  Rectangle {
                    width: parent.width * root.rowShare(modelData)
                    height: parent.height
                    radius: parent.radius
                    color: root.accent
                  }
                }

                Text {
                  id: durationText
                  width: Style.space(70)
                  anchors.right: parent.right
                  anchors.verticalCenter: parent.verticalCenter
                  text: root.formatDuration(Number(modelData.focused_seconds || 0))
                  color: root.dim
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.bodySmall
                  horizontalAlignment: Text.AlignRight
                }
              }
            }
          }

          PanelSeparator {
            foreground: root.foreground
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
              text: "Terminal"
              foreground: root.foreground
              accent: root.accent
              onClicked: root.openTerminalReport()
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
