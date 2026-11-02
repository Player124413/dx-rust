@echo off
setlocal
where cargo-ndk >nul 2>nul || (echo Install cargo-ndk: cargo install cargo-ndk --version 4.1.2 --locked & exit /b 1)
where gradle >nul 2>nul || (echo Install Gradle 8.13 or build from Android Studio. & exit /b 1)
if "%ANDROID_SDK_ROOT%"=="" if "%ANDROID_HOME%"=="" (echo Set ANDROID_SDK_ROOT or ANDROID_HOME. & exit /b 1)
bash android/build_rust.sh || exit /b 1
gradle -p android assembleRelease || exit /b 1
echo APK: android\app\build\outputs\apk\release\app-release.apk
