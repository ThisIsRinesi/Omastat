import QtQuick
Item {
  property QtObject bar: null
  property string moduleName: ""
  property var settings: ({})
  property bool manageIpc: false
  property bool opened: false
  function open() { opened = true }
  function close() { opened = false }
  function toggle() { opened = !opened }
  function setting(name, fallback) {
    var value = settings[name]
    return value === undefined || value === null ? fallback : value
  }
}
