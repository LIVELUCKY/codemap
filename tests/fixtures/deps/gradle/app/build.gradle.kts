plugins { id("sample.app") }

dependencies {
    implementation(project(":features:home"))
    implementation(project(":base:ui"))
}
