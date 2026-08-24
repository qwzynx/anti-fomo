#!/usr/bin/env node
// `src-tauri/gen/android` is gitignored and regenerated from Tauri's template
// by `tauri android init` on every CI run, so a release signingConfig can't
// just be hand-edited into build.gradle.kts once — it has to be reapplied
// after every init. This script does that, reading the key material from
// gen/android/keystore.properties (written by the release workflow from
// GitHub secrets; see .github/workflows/release.yml).
//
// No-op if keystore.properties isn't present, so a local `tauri android
// build` without secrets configured still produces an (unsigned) build
// instead of failing.
import { existsSync, readFileSync, writeFileSync } from "node:fs";

const gradleFile = "app/src-tauri/gen/android/app/build.gradle.kts";
const keystoreProperties = "app/src-tauri/gen/android/keystore.properties";

if (!existsSync(keystoreProperties)) {
  console.log(`${keystoreProperties} not found — leaving the release build unsigned.`);
  process.exit(0);
}

let gradle = readFileSync(gradleFile, "utf8");

function replaceOnce(source, anchor, replacement, label) {
  const count = source.split(anchor).length - 1;
  if (count !== 1) {
    throw new Error(
      `Expected exactly one match for ${label} in ${gradleFile}, found ${count}. ` +
        "The Tauri-generated file may have changed shape — update this script's anchors.",
    );
  }
  return source.replace(anchor, replacement);
}

gradle = replaceOnce(
  gradle,
  `val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {`,
  `val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

val keystoreProperties = Properties().apply {
    val propFile = rootProject.file("keystore.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {`,
  "the tauriProperties block",
);

gradle = replaceOnce(
  gradle,
  `    }
    buildTypes {
        getByName("debug") {`,
  `    }
    signingConfigs {
        create("release") {
            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["password"] as String
            // rootProject, not file(): keystore.properties and the .jks beside
            // it live in gen/android, while this script edits gen/android/app.
            storeFile = rootProject.file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["password"] as String
        }
    }
    buildTypes {
        getByName("debug") {`,
  "the buildTypes block",
);

gradle = replaceOnce(
  gradle,
  `        getByName("release") {
            isMinifyEnabled = true`,
  `        getByName("release") {
            signingConfig = signingConfigs.getByName("release")
            isMinifyEnabled = true`,
  'the getByName("release") block',
);

writeFileSync(gradleFile, gradle);
console.log(`Added release signingConfig to ${gradleFile}.`);
