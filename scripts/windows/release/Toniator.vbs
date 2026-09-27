Option Explicit
' Double-click starts the real application without a console window.
Dim shell, files, root, environment, command, argument
Set shell = CreateObject("WScript.Shell")
Set files = CreateObject("Scripting.FileSystemObject")
root = files.GetParentFolderName(WScript.ScriptFullName)
Set environment = shell.Environment("PROCESS")
environment("PATH") = root & "\bin;" & root & "\media\ffmpeg-8.1.2-full_build\bin;" & environment("PATH")
environment("GTK_A11Y") = "accesskit"
environment("GTK_DATA_PREFIX") = root
environment("GTK_EXE_PREFIX") = root
environment("XDG_DATA_DIRS") = root & "\share"
environment("GSETTINGS_SCHEMA_DIR") = root & "\share\glib-2.0\schemas"
environment("GI_TYPELIB_PATH") = root & "\lib\girepository-1.0"
environment("GDK_PIXBUF_MODULE_FILE") = root & "\lib\gdk-pixbuf-2.0\2.10.0\loaders.cache"
environment("FONTCONFIG_PATH") = root & "\etc\fonts"
environment("FONTCONFIG_FILE") = root & "\etc\fonts\fonts.conf"
command = QuoteArgument(root & "\bin\toniator-app.exe")
For Each argument In WScript.Arguments
    command = command & " " & QuoteArgument(argument)
Next
shell.Run command, 0, False

' Quotes UTF-16 arguments using the Windows native command-line rules.
Function QuoteArgument(value)
    Dim result, slashes, index, character
    result = Chr(34)
    slashes = 0
    For index = 1 To Len(value)
        character = Mid(value, index, 1)
        If character = "\" Then
            slashes = slashes + 1
        ElseIf character = Chr(34) Then
            result = result & String(2 * slashes + 1, "\") & character
            slashes = 0
        Else
            result = result & String(slashes, "\") & character
            slashes = 0
        End If
    Next
    QuoteArgument = result & String(2 * slashes, "\") & Chr(34)
End Function
