#define CLIENT_TELNET 0
#define CLIENT_FFI 1

extern struct DescriptorManager;
extern struct DescriptorId;

struct DescriptorManager* ffi_create_descriptor_manager();
struct DescriptorId* ffi_new_descriptor(struct DescriptorManager* manager, size_t type);
ssize_t ffi_write_to_descriptor(struct DescriptorManager* manager, struct DescriptorId* identifier, const char* content);
ssize_t ffi_read_from_descriptor(struct DescriptorManager* manager, struct DescriptorId* identifier, char* read_point, size_t space_left, size_t* out_read_bytes);
void ffi_close_descriptor(struct DescriptorManager* manager, struct DescriptorId* identifier);
