#version 330 core

out vec4 FragColor;

in vec3 OurColor;
in vec2 TexCoord;

uniform sampler2D Crate;
uniform sampler2D Smiley;

void main() {
    FragColor = mix(texture(Crate, TexCoord), texture(Smiley, TexCoord), 0.2) * vec4(OurColor, 1.0);
}
