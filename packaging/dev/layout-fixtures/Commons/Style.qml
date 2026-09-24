pragma Singleton
import QtQuick
QtObject {
 property int gapsOut: 5
 property var font: ({family: "sans-serif", body:14, bodySmall:12, caption:11, subtitle:16, title:20})
 property var spacing: ({popupPadding:16})
 function space(n) { return n }
}
