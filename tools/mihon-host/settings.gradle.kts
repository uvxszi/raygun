// The host that runs a Mihon extension's own code on desktop (Raygun's sidecar): see
// `docs/extensions/formats/mihon.md`, "Running source code".
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

rootProject.name = "mihon-host"
