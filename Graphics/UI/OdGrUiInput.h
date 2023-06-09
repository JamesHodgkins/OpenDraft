#pragma once

#include <unordered_map>
#include "glfw/glfw3.h"

#include <iostream>


class OdGrKeyInput
{
private:
	bool down = false;
	bool pressed = false;
	bool released = false;

public:

	void changeState(bool newState)
	{
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

	std::unordered_map<int, OdGrKeyInput>keys;
	struct MouseStruct
	{
		OdGrKeyInput leftButton;
		OdGrKeyInput middleButton;
		OdGrKeyInput rightButton;
		PositionStruct position;
	} mouse;

	GrInputMap()
	{
		keys = {
			{GLFW_KEY_A, OdGrKeyInput()},
			{GLFW_KEY_B, OdGrKeyInput()},
			{GLFW_KEY_C, OdGrKeyInput()},
			{GLFW_KEY_D, OdGrKeyInput()},
			{GLFW_KEY_E, OdGrKeyInput()},
			{GLFW_KEY_F, OdGrKeyInput()},
			{GLFW_KEY_G, OdGrKeyInput()},
			{GLFW_KEY_H, OdGrKeyInput()},
			{GLFW_KEY_I, OdGrKeyInput()},
			{GLFW_KEY_J, OdGrKeyInput()},
			{GLFW_KEY_K, OdGrKeyInput()},
			{GLFW_KEY_L, OdGrKeyInput()},
			{GLFW_KEY_M, OdGrKeyInput()},
			{GLFW_KEY_N, OdGrKeyInput()},
			{GLFW_KEY_O, OdGrKeyInput()},
			{GLFW_KEY_P, OdGrKeyInput()},
			{GLFW_KEY_Q, OdGrKeyInput()},
			{GLFW_KEY_R, OdGrKeyInput()},
			{GLFW_KEY_S, OdGrKeyInput()},
			{GLFW_KEY_T, OdGrKeyInput()},
			{GLFW_KEY_U, OdGrKeyInput()},
			{GLFW_KEY_V, OdGrKeyInput()},
			{GLFW_KEY_W, OdGrKeyInput()},
			{GLFW_KEY_X, OdGrKeyInput()},
			{GLFW_KEY_Y, OdGrKeyInput()},
			{GLFW_KEY_Z, OdGrKeyInput()},
			{GLFW_KEY_0, OdGrKeyInput()},
			{GLFW_KEY_1, OdGrKeyInput()},
			{GLFW_KEY_2, OdGrKeyInput()},
			{GLFW_KEY_3, OdGrKeyInput()},
			{GLFW_KEY_4, OdGrKeyInput()},
			{GLFW_KEY_5, OdGrKeyInput()},
			{GLFW_KEY_6, OdGrKeyInput()},
			{GLFW_KEY_7, OdGrKeyInput()},
			{GLFW_KEY_8, OdGrKeyInput()},
			{GLFW_KEY_9, OdGrKeyInput()}
		};
	}


	

};



	