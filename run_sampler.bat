@echo off
REM Wrapper to run the sampler avoiding terminal ';' injection issues.
REM Args: %1=Label %2=full|narrow %3=Load(overwrite|createdelete) %4=filesPerBurst %5=intervalMs
setlocal
cd /d c:\Users\10031\Desktop\sousuo\EasySearch
set MASKARG=
if /I "%2"=="full" set MASKARG=-FullMask
set FILES=%4
if "%FILES%"=="" set FILES=200
set INTERVAL=%5
if "%INTERVAL%"=="" set INTERVAL=500
powershell -ExecutionPolicy Bypass -NoProfile -File c:\Users\10031\Desktop\sousuo\EasySearch\usn_cpu_probe.ps1 -Label %1 -WindowSecs 30 -Drives C -Load %3 -LoadFilesPerBurst %FILES% -BurstIntervalMs %INTERVAL% %MASKARG%
echo SAMPLER_DONE_%1 > sampler_done_%1.txt
endlocal
