@echo off
setlocal
set "PATH=%~dp0bin;%~dp0media\ffmpeg-8.1.2-full_build\bin;%PATH%"
set "XDG_DATA_DIRS=%~dp0share"
set "GSETTINGS_SCHEMA_DIR=%~dp0share\glib-2.0\schemas"
set "FONTCONFIG_PATH=%~dp0etc\fonts"
set "FONTCONFIG_FILE=%~dp0etc\fonts\fonts.conf"
if "%~1"=="" (
  "%~dp0bin\toniator.exe" --help
) else (
  "%~dp0bin\toniator.exe" %*
)
exit /b %errorlevel%
