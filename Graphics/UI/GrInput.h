#pragma once

class GrKeyInput
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

	struct MouseStruct
	{
		GrKeyInput leftButton;
		GrKeyInput middleButton;
		GrKeyInput rightButton;
		PositionStruct position;
	};
public:

	struct Keys
	{
		GrKeyInput aKey;
	};

	MouseStruct mouse;
	Keys keys;
};