#include <Security/Security.h>
#include <Security/cssmapple.h>
#include <CommonCrypto/CommonDigest.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Only disposable, synthetic items may enter this investigation harness.
static int permitted(const char *service) {
    const char *prefix = "loopflow-LOO-661-explicit-probe-";
    return strncmp(service, prefix, strlen(prefix)) == 0;
}

static void digest_data(CC_SHA256_CTX *ctx, const CSSM_DATA *data) {
    CC_SHA256_Update(ctx, &data->Length, sizeof(data->Length));
    if (data->Length) CC_SHA256_Update(ctx, data->Data, (CC_LONG)data->Length);
}

static void digest_list(CC_SHA256_CTX *ctx, const CSSM_LIST *list) {
    CC_SHA256_Update(ctx, &list->ListType, sizeof(list->ListType));
    for (CSSM_LIST_ELEMENT *e = list->Head; e; e = e->NextElement) {
        CC_SHA256_Update(ctx, &e->ElementType, sizeof(e->ElementType));
        if (e->ElementType == CSSM_LIST_ELEMENT_WORDID)
            CC_SHA256_Update(ctx, &e->WordID, sizeof(e->WordID));
        if (e->ElementType == CSSM_LIST_ELEMENT_SUBLIST) digest_list(ctx, &e->Element.Sublist);
        if (e->ElementType == CSSM_LIST_ELEMENT_DATUM) digest_data(ctx, &e->Element.Word);
    }
    unsigned end = 0xffffffff;
    CC_SHA256_Update(ctx, &end, sizeof(end));
}

static OSStatus snapshot(SecAccessRef access) {
    CSSM_ACL_OWNER_PROTOTYPE *owner = NULL;
    CSSM_ACL_ENTRY_INFO *entries = NULL;
    uint32 count = 0;
    OSStatus status = SecAccessGetOwnerAndACL(access, &owner, &count, &entries);
    if (status) return status;
    CC_SHA256_CTX ctx;
    CC_SHA256_Init(&ctx);
    int partition_count = 0, apple_tool = 0, decoded = 0;
    digest_list(&ctx, &owner->TypedSubject);
    CC_SHA256_Update(&ctx, &owner->Delegate, sizeof(owner->Delegate));
    CC_SHA256_Update(&ctx, &count, sizeof(count));
    for (uint32 i = 0; i < count; i++) {
        CSSM_ACL_ENTRY_PROTOTYPE *p = &entries[i].EntryPublicInfo;
        for (uint32 j = 0; j < p->Authorization.NumberOfAuthTags; j++) {
            if (p->Authorization.AuthTags[j] != CSSM_ACL_AUTHORIZATION_PARTITION_ID) continue;
            partition_count++;
            CSSM_LIST_ELEMENT *head = p->TypedSubject.Head;
            CSSM_LIST_ELEMENT *payload = head ? head->NextElement : NULL;
            if (!payload || payload->ElementType != CSSM_LIST_ELEMENT_DATUM) continue;
            CFDataRef data = CFDataCreate(NULL, payload->Element.Word.Data, payload->Element.Word.Length);
            CFPropertyListRef plist = CFPropertyListCreateWithData(NULL, data, 0, NULL, NULL);
            if (plist && CFGetTypeID(plist) == CFDictionaryGetTypeID()) {
                CFArrayRef partitions = CFDictionaryGetValue(plist, CFSTR("Partitions"));
                if (partitions && CFGetTypeID(partitions) == CFArrayGetTypeID()) {
                    decoded++;
                    apple_tool |= CFArrayContainsValue(partitions,
                        CFRangeMake(0, CFArrayGetCount(partitions)), CFSTR("apple-tool:"));
                }
            }
            if (plist) CFRelease(plist);
            CFRelease(data);
        }
        digest_list(&ctx, &p->TypedSubject);
        CC_SHA256_Update(&ctx, &p->Delegate, sizeof(p->Delegate));
        CC_SHA256_Update(&ctx, &p->Authorization.NumberOfAuthTags, sizeof(p->Authorization.NumberOfAuthTags));
        CC_SHA256_Update(&ctx, p->Authorization.AuthTags,
            p->Authorization.NumberOfAuthTags * sizeof(*p->Authorization.AuthTags));
        digest_data(&ctx, &p->TimeRange.StartDate);
        digest_data(&ctx, &p->TimeRange.EndDate);
        CC_SHA256_Update(&ctx, p->EntryTag, (CC_LONG)strnlen(p->EntryTag, sizeof(p->EntryTag)));
    }
    unsigned char digest[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256_Final(digest, &ctx);
    printf("{\"partition_count\":%d,\"decoded\":%d,\"apple_tool\":%s,\"digest\":\"",
        partition_count, decoded, apple_tool ? "true" : "false");
    for (unsigned i = 0; i < sizeof(digest); i++) printf("%02x", digest[i]);
    printf("\"}\n");
    // This single-snapshot child exits immediately, reclaiming the CSSM copies.
    return 0;
}

static OSStatus reconstruct(SecAccessRef *access, int strip_partition, int strip_integrity) {
    CSSM_ACL_OWNER_PROTOTYPE *owner = NULL;
    CSSM_ACL_ENTRY_INFO *entries = NULL;
    uint32 count = 0, kept = 0;
    OSStatus status = SecAccessGetOwnerAndACL(*access, &owner, &count, &entries);
    if (status) return status;
    CSSM_ACL_ENTRY_INFO *filtered = calloc(count, sizeof(*filtered));
    if (!filtered) return errSecAllocate;
    for (uint32 i = 0; i < count; i++) {
        CSSM_AUTHORIZATIONGROUP auth = entries[i].EntryPublicInfo.Authorization;
        int omit = 0;
        for (uint32 j = 0; j < auth.NumberOfAuthTags; j++) {
            omit |= strip_partition && auth.AuthTags[j] == CSSM_ACL_AUTHORIZATION_PARTITION_ID;
            omit |= strip_integrity && auth.AuthTags[j] == CSSM_ACL_AUTHORIZATION_INTEGRITY;
        }
        if (!omit) filtered[kept++] = entries[i];
    }
    SecAccessRef rebuilt = NULL;
    status = SecAccessCreateFromOwnerAndACL(owner, kept, filtered, &rebuilt);
    if (!status) {
        CFRelease(*access);
        *access = rebuilt;
    }
    free(filtered);
    // The bounded child reclaims the copied CSSM structures on exit.
    return status;
}

int main(int argc, char **argv) {
    if (argc < 3 || !permitted(argv[2])) return 2;
    OSStatus status = SecKeychainSetUserInteractionAllowed(false);
    if (status) return 3;
    const char *account = "probe";
    SecKeychainItemRef original = NULL, created = NULL;
    SecAccessRef access = NULL;
    status = SecKeychainFindGenericPassword(NULL, (UInt32)strlen(argv[2]), argv[2],
        (UInt32)strlen(account), account, NULL, NULL, &original);
    if (!status) status = SecKeychainItemCopyAccess(original, &access);
    if (!strcmp(argv[1], "snapshot")) {
        if (!status) status = snapshot(access);
    } else {
        if (argc != 5 || !permitted(argv[3])) return 4;
        unsigned size = (unsigned)strtoul(argv[4], NULL, 10);
        if (size != 2048 && size != 8192) return 5;
        if (!status && !strncmp(argv[1], "rebuild_", 8))
            status = reconstruct(&access,
                !strcmp(argv[1], "rebuild_without_partition") || !strcmp(argv[1], "rebuild_without_both"),
                !strcmp(argv[1], "rebuild_without_integrity") || !strcmp(argv[1], "rebuild_without_both"));
        if (!status && !strcmp(argv[1], "explicit_security")) {
            CFRelease(access);
            access = NULL;
            SecTrustedApplicationRef app = NULL;
            status = SecTrustedApplicationCreateFromPath("/usr/bin/security", &app);
            if (!status) {
                CFArrayRef apps = CFArrayCreate(NULL, (const void **)&app, 1, &kCFTypeArrayCallBacks);
                status = SecAccessCreate(CFSTR("LOO-661 synthetic"), apps, &access);
                CFRelease(apps);
                CFRelease(app);
            }
        }
        char *data = malloc(size);
        if (!data) return 6;
        memset(data, 'x', size);
        memcpy(data, "{\"padding\":\"", 12);
        memcpy(data + size - 2, "\"}", 2);
        SecKeychainAttribute attrs[] = {
            {kSecServiceItemAttr, (UInt32)strlen(argv[3]), argv[3]},
            {kSecAccountItemAttr, (UInt32)strlen(account), (void *)account}
        };
        SecKeychainAttributeList list = {2, attrs};
        OSStatus preparation = status;
        if (!status) status = SecKeychainItemCreateFromContent(kSecGenericPasswordItemClass,
            &list, size, data, NULL, access, &created);
        printf("{\"prepare\":%d,\"create\":%d}\n", (int)preparation, (int)status);
        free(data);
    }
    if (created) CFRelease(created);
    if (access) CFRelease(access);
    if (original) CFRelease(original);
    return !strcmp(argv[1], "snapshot") && status ? 7 : 0;
}
