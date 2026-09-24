import QtQuick
Item {
 id: window
 property var anchorItem
 property var owner
 property var bar
 property bool open
 property bool islandStyle
 property bool richGraphs
 property bool reduceMotion
 property int padding
 property bool centerOnBar
 property int margin
 property int gap
 property var focusTarget
 property int contentWidth
 property int contentHeight
 default property alias panelContent: holder.data
 property alias contentContainer: holder
 width: contentWidth; height: contentHeight
 function fittedContentWidth(w) { return Math.min(w, parent.width) }
 function fittedContentHeight(h, cap) { return Math.min(h, cap, parent.height) }
 Item { id: holder; anchors.fill: parent; anchors.margins: window.padding }
}
