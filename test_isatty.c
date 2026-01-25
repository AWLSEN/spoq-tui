#include <stdio.h>
#include <unistd.h>

int main() {
    printf("isatty(0) = %d\n", isatty(0));
    printf("isatty(1) = %d\n", isatty(1));
    printf("isatty(2) = %d\n", isatty(2));
    printf("ttyname(0) = %s\n", ttyname(0) ? ttyname(0) : "NULL");
    printf("ttyname(1) = %s\n", ttyname(1) ? ttyname(1) : "NULL");
    printf("ttyname(2) = %s\n", ttyname(2) ? ttyname(2) : "NULL");
    fflush(stdout);
    sleep(3);
    return 0;
}
