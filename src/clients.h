#define CLIENT_TELNET 0
#define CLIENT_FFI 1

extern struct Descriptor;

struct Descriptor* ffi_new_descriptor(size_t type);
size_t ffi_write_to_descriptor(struct Descriptor* descriptor, const char* content);
size_t ffi_read_from_descriptor(struct Descriptor* descriptor, char* read_point, size_t space_left);
void ffi_close_descriptor(struct Descriptor* descriptor);
