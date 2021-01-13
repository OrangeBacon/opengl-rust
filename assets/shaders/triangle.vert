#version 330 core

layout (location = 0) in vec3 Position;
layout (location = 1) in vec2 TexCoord;

out VS_OUTPUT {
    vec2 TexCoord;
} OUT;

uniform mat4 model;
uniform mat4 view;
uniform mat4 projection;

void main() {
    gl_Position = projection * view * model * vec4(Position, 1.0);
    OUT.TexCoord = TexCoord;
}
