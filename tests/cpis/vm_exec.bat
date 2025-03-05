@echo off
set VM_NAME=OmniVM
set MEMORY_MB=2048
set CPU_COUNT=2
set OS_TYPE=Ubuntu_64
set BOX_PATH=C:\Users\redst\Downloads\trusty-server-cloudimg-amd64-vagrant-disk1.box
set EXTRACTED_DIR=%TEMP%\vagrant_box_extracted
set API_ENDPOINT=http://localhost:8081/vms/action

echo Extracting Vagrant box...
mkdir %EXTRACTED_DIR% 2>nul
tar -xf %BOX_PATH% -C %EXTRACTED_DIR%

echo Finding virtual disk file...
for %%i in (%EXTRACTED_DIR%\*.vmdk %EXTRACTED_DIR%\*.vdi %EXTRACTED_DIR%\*.vhd) do (
    set DISK_PATH=%%i
    echo Found disk: %%i
    goto disk_found
)
:disk_found

echo Creating VM...
(
echo {
echo   "provider": "virtualbox_cpi_windows",
echo   "action": "create_vm",
echo   "params": {
echo     "vm_name": "%VM_NAME%",
echo     "os_type": "%OS_TYPE%",
echo     "memory_mb": "%MEMORY_MB%",
echo     "cpu_count": "%CPU_COUNT%"
echo   }
echo }
) > "%TEMP%\vm_request.json"

curl -s -X POST -H "Content-Type: application/json" -d @"%TEMP%\vm_request.json" %API_ENDPOINT%

echo Attaching disk...
(
echo {
echo   "provider": "virtualbox_cpi_windows",
echo   "action": "attach_disk",
echo   "params": {
echo     "vm_name": "%VM_NAME%",
echo     "controller_name": "SATAController",
echo     "port": "0",
echo     "disk_path": "%DISK_PATH:\=\\%"
echo   }
echo }
) > "%TEMP%\attach_disk.json"

curl -s -X POST -H "Content-Type: application/json" -d @"%TEMP%\attach_disk.json" %API_ENDPOINT%

echo Starting VM...
(
echo {
echo   "provider": "virtualbox_cpi_windows",
echo   "action": "start_vm",
echo   "params": {
echo     "vm_name": "%VM_NAME%"
echo   }
echo }
) > "%TEMP%\start_vm.json"

curl -s -X POST -H "Content-Type: application/json" -d @"%TEMP%\start_vm.json" %API_ENDPOINT%

echo Waiting 90 seconds for VM to boot...
timeout /t 1 /nobreak > nul

echo Executing command in VM...
(
echo {
echo   "provider": "virtualbox_cpi_windows",
echo   "action": "execute_command",
echo   "params": {
echo     "vm_name": "%VM_NAME%",
echo     "username": "vagrant",
echo     "password": "vagrant",
echo     "command_path": "/bin/ls",
echo     "command_args": "-la /"
echo   }
echo }
) > "%TEMP%\execute_command.json"

echo Debug: Executing command JSON:
type "%TEMP%\execute_command.json"
echo.

curl -v -X POST -H "Content-Type: application/json" -d @"%TEMP%\execute_command.json" %API_ENDPOINT%
echo Script completed.