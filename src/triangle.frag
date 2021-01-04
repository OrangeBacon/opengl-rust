#version 330 core

out vec4 Color;
in vec3 our_color;

void main() {
    Color = vec4(our_color, 1.0f);
}
