#include <stddef.h>
struct DescriptorManager; // eg for Telnet descriptors, monitors new telnet connections and returns them via new_descriptor
struct Descriptor; // the handle for IO (ie a Telnet socket)


struct DescriptorManager* new_descriptor_manager(unsigned short int port);
int close_descriptor_manager(struct DescriptorManager *manager);

struct Descriptor* new_descriptor(struct DescriptorManager *manager);
int close_descriptor(struct DescriptorManager *manager, struct Descriptor *descriptor);

/*
 * write_to_descriptor takes a descriptor, and text to write to the
 * descriptor.  It keeps calling the system-level write() until all
 * the text has been delivered to the OS, or until an error is
 * encountered.
 *
 * Returns:
 * >=0  If all is well and good.
 *  -1  If an error was encountered, so that the player should be cut off.
 */
size_t write_to_descriptor(struct DescriptorManager *manager, struct Descriptor *descriptor, const char *content);
/*
int write_to_descriptor(socket_t desc, const char *txt)
{
  ssize_t bytes_written;
  size_t total = strlen(txt), write_total = 0;

  while (total > 0) {
    bytes_written = perform_socket_write(desc, txt, total);

    if (bytes_written < 0) {
       Fatal error.  Disconnect the player.
      perror("SYSERR: Write to socket");
      return (-1);
    } else if (bytes_written == 0) {
      Temporary failure -- socket buffer full.
      return (write_total);
    } else {
      txt += bytes_written;
      total -= bytes_written;
      write_total += bytes_written;
    }
  }

  return (write_total);
}
*/
/*
 * perform_socket_write: takes a descriptor, a pointer to text, and a
 * text length, and tries once to send that text to the OS.  This is
 * where we stuff all the platform-dependent stuff that used to be
 * ugly #ifdef's in write_to_descriptor().
 *
 * This function must return:
 *
 * -1  If a fatal error was encountered in writing to the descriptor.
 *  0  If a transient failure was encountered (e.g. socket buffer full).
 * >0  To indicate the number of bytes successfully written, possibly
 *     fewer than the number the caller requested be written.
 *
 * Right now there are two versions of this function: one for Windows,
 * and one for all other platforms.
 */
/*
#if defined(CIRCLE_WINDOWS)

ssize_t perform_socket_write(socket_t desc, const char *txt, size_t length)
{
  ssize_t result;

  result = send(desc, txt, length, 0);

  if (result > 0) {
     Write was sucessful
    return (result);
  }

  if (result == 0) {
    This should never happen!
    log("SYSERR: Huh??  write() returned 0???  Please report this!");
    return (-1);
  }

  result < 0: An error was encountered.

  Transient error?
  if (WSAGetLastError() == WSAEWOULDBLOCK || WSAGetLastError() == WSAEINTR)
    return (0);

  Must be a fatal error.
  return (-1);
}

#else

#if defined(CIRCLE_ACORN)
#define write	socketwrite
#endif

perform_socket_write for all Non-Windows platforms
ssize_t perform_socket_write(socket_t desc, const char *txt, size_t length)
{
  ssize_t result;

  result = write(desc, txt, length);

  if (result > 0) {
    Write was successful.
    return (result);
  }

  if (result == 0) {
    This should never happen!
    log("SYSERR: Huh??  write() returned 0???  Please report this!");
    return (-1);
  }

   * result < 0, so an error was encountered - is it transient?
   * Unfortunately, different systems use different constants to
   * indicate this.

#ifdef EAGAIN		POSIX
  if (errno == EAGAIN)
    return (0);
#endif

#ifdef EWOULDBLOCK	BSD
  if (errno == EWOULDBLOCK)
    return (0);
#endif

#ifdef EDEADLK		Macintosh
  if (errno == EDEADLK)
    return (0);
#endif

  Looks like the error was fatal.  Too bad.
  return (-1);
}

#endif CIRCLE_WINDOWS
*/

size_t read_from_descriptor(struct DescriptorManager *manager, struct Descriptor *descriptor, char *read_point, size_t space_left);
/*
 * Same information about perform_socket_write applies here. I like
 * standards, there are so many of them. -gg 6/30/98
 */
/*
ssize_t read_from_descriptor(socket_t desc, char *read_point, size_t space_left)
{
  ssize_t ret;

#if defined(CIRCLE_ACORN)
  ret = recv(desc, read_point, space_left, MSG_DONTWAIT);
#elif defined(CIRCLE_WINDOWS)
  ret = recv(desc, read_point, space_left, 0);
#else
  ret = read(desc, read_point, space_left);
#endif

  Read was successful.
  if (ret > 0)
    return (ret);

  read() returned 0, meaning we got an EOF.
  if (ret == 0) {
    log("WARNING: EOF on socket read (connection broken by peer)");
    return (-1);
  }

   * read returned a value < 0: there was an error

#if defined(CIRCLE_WINDOWS)	 Windows
  if (WSAGetLastError() == WSAEWOULDBLOCK || WSAGetLastError() == WSAEINTR)
    return (0);
#else

#ifdef EINTR		Interrupted system call - various platforms
  if (errno == EINTR)
    return (0);
#endif

#ifdef EAGAIN		POSIX
  if (errno == EAGAIN)
    return (0);
#endif

#ifdef EWOULDBLOCK	BSD
  if (errno == EWOULDBLOCK)
    return (0);
#endif EWOULDBLOCK

#ifdef EDEADLK		Macintosh
  if (errno == EDEADLK)
    return (0);
#endif

#ifdef ECONNRESET
  if (errno == ECONNRESET)
    return (-1);
#endif

#endif CIRCLE_WINDOWS

   * We don't know what happened, cut them off. This qualifies for
   * a SYSERR because we have no idea what happened at this point.
  perror("SYSERR: perform_socket_read: about to lose connection");
  return (-1);
}
*/
