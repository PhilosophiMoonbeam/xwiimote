/*
 * WiiLand - tools
 * Written 2010, 2011 by David Herrmann
 * Dedicated to the Public Domain
 */

/*
 * WiiLand EEPROM Dump
 * This tool reads the whole eeprom of a wiimote and dumps the output to
 * stdout. This requires debugfs support in the kernel and the hid-wiimote
 * kernel module. The caller must have permission to read the eeprom file.
 *
 * Debugfs compiled:
 *   zgrep DEBUG_FS /proc/config.gz
 * Mount debugfs:
 *   mount -t debugfs debugfs /sys/kernel/debug
 * Path to eeprom file:
 *   /sys/kernel/debug/hid/<dev>/eeprom
 */

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void usage(const char *prog, FILE *stream)
{
	fprintf(stream, "Usage: %s FILE\n", prog);
	fprintf(stream, "Read a Wii Remote EEPROM file and write its contents to stdout.\n");
}

static void show(const char *buf, size_t len)
{
	size_t i;

	for (i = 0; i < len; ++i)
		printf(" 0x%02hhx", buf[i]);
}

static ssize_t read_retry(int fd, void *buf, size_t len)
{
	ssize_t ret;

	do
		ret = read(fd, buf, len);
	while (ret < 0 && errno == EINTR);

	return ret;
}

static int dump(int fd, const char *file)
{
	char buf[1];
	ssize_t ret;
	size_t off, i;

	off = 0;
	while (1) {
		printf("0x%08zu:", off);

		for (i = 0; i < 8; ++i) {
			ret = read_retry(fd, buf, sizeof(buf));
			if (ret > 0) {
				show(buf, ret);
			} else if (ret < 0) {
				int error;

				error = errno;
				/* Keep the established stdout dump format. */
				printf(" (read error %d)", error);
				fprintf(stderr,
					"Cannot read eeprom file '%s' at offset 0x%08zx: %s\n",
					file, off, strerror(error));
				return EXIT_FAILURE;
			} else {
				printf(" (eof)");
				if (i != 0) {
					fprintf(stderr,
						"Unexpected end of eeprom file '%s' at offset 0x%08zx\n",
						file, off);
					return EXIT_FAILURE;
				}
				return EXIT_SUCCESS;
			}
			++off;
		}
		printf("\n");
	}
}

static int open_eeprom(const char *file)
{
	int fd;

	do
		fd = open(file, O_RDONLY);
	while (fd < 0 && errno == EINTR);

	if (fd < 0)
		fprintf(stderr, "Cannot open eeprom file '%s': %s\n",
			file, strerror(errno));

	return fd;
}

int main(int argc, char **argv)
{
	int fd;
	int status;

	if (argc == 2 && (!strcmp(argv[1], "-h") || !strcmp(argv[1], "--help"))) {
		usage(argv[0], stdout);
		return EXIT_SUCCESS;
	}

	if (argc != 2 || !*argv[1]) {
		usage(argv[0], stderr);
		return EXIT_FAILURE;
	}

	fd = open_eeprom(argv[1]);
	if (fd < 0)
		return EXIT_FAILURE;

	status = dump(fd, argv[1]);
	if (close(fd) < 0) {
		fprintf(stderr, "Cannot close eeprom file '%s': %s\n",
			argv[1], strerror(errno));
		return EXIT_FAILURE;
	}

	return status;
}
