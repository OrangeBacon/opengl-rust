#version 330 core

layout (location = 0) in vec3 Position;
layout (location = 1) in vec3 Color;
layout (location = 2) in vec2 TexCoord;

out VS_OUTPUT {
    vec3 Color;
    vec2 TexCoord;
} OUT;

uniform mat4 transform;

void main() {
    gl_Position = transform * vec4(Position, 1.0);
    OUT.Color = Color;
    OUT.TexCoord = TexCoord;
}
