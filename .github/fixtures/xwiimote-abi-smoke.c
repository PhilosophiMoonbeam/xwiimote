#include <xwiimote.h>

#include <stdlib.h>
#include <string.h>

int main(void)
{
	const char *name = xwii_get_iface_name(XWII_IFACE_CORE);
	struct xwii_monitor *monitor;
	char *path;
	int status = EXIT_SUCCESS;

	if (name == NULL || strcmp(name, XWII_NAME_CORE) != 0)
		return EXIT_FAILURE;

	monitor = xwii_monitor_new(false, false);
	if (monitor == NULL)
		return EXIT_FAILURE;

	if (xwii_monitor_get_fd(monitor, false) != -1)
		status = EXIT_FAILURE;

	path = xwii_monitor_poll(monitor);
	free(path);
	xwii_monitor_unref(monitor);
	return status;
}
