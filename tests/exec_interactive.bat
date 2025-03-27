@echo off
setlocal enabledelayedexpansion

:: Set your API endpoint
set "API_ENDPOINT=http://localhost:8081/vms/action"

echo Remote Terminal Client
echo ---------------------
echo Type 'exit' to quit
echo.

:: Create Python parser once (instead of every command)
echo import json > parse_output.py
echo import sys >> parse_output.py
echo with open('response.json', 'r') as f: >> parse_output.py
echo     data = json.load(f) >> parse_output.py
echo if data.get('success'): >> parse_output.py
echo     output = data.get('result', {}).get('output', '') >> parse_output.py
echo     print(output.replace('\\n', '\n')) >> parse_output.py
echo else: >> parse_output.py
echo     print(f"Error: {data.get('error')}") >> parse_output.py

:commandloop
set "command="
set /p "command=$ "

if /i "%command%"=="exit" (
    del parse_output.py
    goto :eof
)

:: Parse the command
for /f "tokens=1,* delims= " %%a in ("%command%") do (
    set "cmd_name=%%a"
    set "cmd_args=%%b"
)
set "command_path=/bin/!cmd_name!"

:: Use a single curl call with inline JSON (no temp file for request)
curl -s -X POST -H "Content-Type: application/json" -d "{\"provider\":\"virtualbox_cpi_linux\",\"action\":\"execute_command\",\"params\":{\"worker_name\":\"OmniVM\",\"username\":\"vagrant\",\"password\":\"vagrant\",\"command_path\":\"!command_path!\",\"command_args\":\"!cmd_args!\"}}" "%API_ENDPOINT%" > response.json

:: Process and display output directly
python parse_output.py

:: Clean up response file immediately
del response.json

goto commandloop