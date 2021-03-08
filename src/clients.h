#define CLIENT_TELNET 0
#define CLIENT_FFI 1

size_t ffi_write_to_descriptor(socket_t* descriptor, const char* content);
size_t ffi_read_from_descriptor(socket_t* descriptor, char* read_point, size_t space_left);
void ffi_close_descriptor(socket_t* descriptor);
