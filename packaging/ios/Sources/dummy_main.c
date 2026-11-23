/* Xcode needs a linkable _main so the Debug build can finish linking.
   The post-build script then replaces this executable with the Bevy
   `apa-kan` binary (which already contains the thin AppDelegate). */
int main(int argc, char *argv[])
{
	(void)argc;
	(void)argv;
	return 0;
}
