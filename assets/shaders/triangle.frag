#version 330 core

in VS_OUTPUT {
    vec2 TexCoord;
} IN;

out vec4 Color;

uniform sampler2D crate;
uniform sampler2D face;

void main() {
    Color = mix(texture(crate, IN.TexCoord), texture(face, IN.TexCoord), 0.2);
}
