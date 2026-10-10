@echo off
rem 编译 casc_tool.exe（使用 D 盘的 MSVC 工具集 + C 盘的 Windows SDK）
rem 用法: build_casc_tool.cmd
setlocal
set MSVC=D:\VS2022\VC\Tools\MSVC\14.44.35207
set SDK=C:\Program Files (x86)\Windows Kits\10
set SDKV=10.0.26100.0
set PATH=%MSVC%\bin\HostX64\x64;%PATH%
set INCLUDE=%MSVC%\include;%SDK%\Include\%SDKV%\ucrt;%SDK%\Include\%SDKV%\um;%SDK%\Include\%SDKV%\shared;%SDK%\Include\%SDKV%\winrt
set LIB=%MSVC%\lib\x64;%SDK%\Lib\%SDKV%\ucrt\x64;%SDK%\Lib\%SDKV%\um\x64
cd /d "%~dp0"
cl /nologo /O2 /W3 /utf-8 /Fe:casc_tool.exe casc_tool.c
if errorlevel 1 (echo BUILD FAILED & exit /b 1)
echo BUILD OK: %~dp0casc_tool.exe
