@echo off
set VM_NAME=OmniVM
set ISO_PATH=C:\\Users\\redst\\Downloads\\BookwormPup64_10.0.10.iso
rem Convert single backslashes to double backslashes
set "ISO_PATH_JSON=%ISO_PATH:\=\\%"

(
echo {
echo   "provider": "virtualbox_cpi_windows",
echo   "action": "create_and_boot",
echo   "params": {
echo     "vm_name": "%VM_NAME%",
echo     "os_type": "Ubuntu_64",
echo     "memory_mb": "2048",
echo     "cpu_count": "2",
echo     "iso_path": "%ISO_PATH_JSON%"
echo   }
echo }
) > "%TEMP%\vm_request.json"

curl -s -X POST -H "Content-Type: application/json" -d @"%TEMP%\vm_request.json" http://localhost:8081/vms/action