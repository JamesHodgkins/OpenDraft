#pragma once

#include <unordered_map>
#include "glfw/glfw3.h"


class GrKeyInput
{
private:
	bool down = false;
	bool pressed = false;
	bool released = false;

public:

	void changeState(bool newState)
	{
		if (newState == true)
		{
			std::cout << "state = true" << std::endl;
		}
		else
		{
			std::cout << "state = false" << std::endl;
		}

		pressed = (!down && newState);
		released = (down && !newState);
		down = newState;
	}

	bool isDown() const { return down; }
	bool isPressed() const { return pressed; }
	bool isReleased() const { return released; }
};


class GrInputMap
{
private:
	struct PositionStruct
	{
		int y;
		int x;
	} position;

	
public:

	std::unordered_map<int, GrKeyInput>keys;
	struct MouseStruct
	{
		GrKeyInput leftButton;
		GrKeyInput middleButton;
		GrKeyInput rightButton;
		PositionStruct position;
	} mouse;

	GrInputMap()
	{
		keys = {
			{GLFW_KEY_A, GrKeyInput()},
			{GLFW_KEY_B, GrKeyInput()},
			{GLFW_KEY_C, GrKeyInput()},
			{GLFW_KEY_D, GrKeyInput()},
			{GLFW_KEY_E, GrKeyInput()},
			{GLFW_KEY_F, GrKeyInput()},
			{GLFW_KEY_G, GrKeyInput()},
			{GLFW_KEY_H, GrKeyInput()},
			{GLFW_KEY_I, GrKeyInput()},
			{GLFW_KEY_J, GrKeyInput()},
			{GLFW_KEY_K, GrKeyInput()},
			{GLFW_KEY_L, GrKeyInput()},
			{GLFW_KEY_M, GrKeyInput()},
			{GLFW_KEY_N, GrKeyInput()},
			{GLFW_KEY_O, GrKeyInput()},
			{GLFW_KEY_P, GrKeyInput()},
			{GLFW_KEY_Q, GrKeyInput()},
			{GLFW_KEY_R, GrKeyInput()},
			{GLFW_KEY_S, GrKeyInput()},
			{GLFW_KEY_T, GrKeyInput()},
			{GLFW_KEY_U, GrKeyInput()},
			{GLFW_KEY_V, GrKeyInput()},
			{GLFW_KEY_W, GrKeyInput()},
			{GLFW_KEY_X, GrKeyInput()},
			{GLFW_KEY_Y, GrKeyInput()},
			{GLFW_KEY_Z, GrKeyInput()},
			{GLFW_KEY_0, GrKeyInput()},
			{GLFW_KEY_1, GrKeyInput()},
			{GLFW_KEY_2, GrKeyInput()},
			{GLFW_KEY_3, GrKeyInput()},
			{GLFW_KEY_4, GrKeyInput()},
			{GLFW_KEY_5, GrKeyInput()},
			{GLFW_KEY_6, GrKeyInput()},
			{GLFW_KEY_7, GrKeyInput()},
			{GLFW_KEY_8, GrKeyInput()},
			{GLFW_KEY_9, GrKeyInput()}
		};
	}


	

};



	