# GameActivity JNI + the Rust cdylib. Keep names even if minify is turned on later.
-keep class com.google.androidgamesdk.** { *; }
-keep class io.ramate.apakan.MainActivity { *; }
-dontwarn com.google.androidgamesdk.**
