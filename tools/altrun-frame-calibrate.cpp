// Isolated desktop-frame experiment. No launcher, input or visible window.
#include <cstdio>

int main(int, char**) {
    std::fputs("Controlled GPU pixel readback is not implemented\n", stderr);
    return 2;
}
