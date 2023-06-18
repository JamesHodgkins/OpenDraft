#pragma once

/**************************************************************************************
* OpenDraft:    GUI Window Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrWindow.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI base window class                                                           *
***************************************************************************************/



#include <vector>
#include <iostream>

#define Window GLFWwindow			// Alias Window for GLFWwindow
#define Context NVGcontext			// Alias Window for GLFWwindow

#include "System/OdSyCore.h"
#include "Graphics/UI/OdGrUiComponent.h"
#include "Graphics/OdGrDraw.h"


// Class representing a window
class OdGrUiWindow : public OdGrUiComponent
{
protected:
	GLFWwindow* glfwHandle = nullptr;		// Handle to the GLFW window
	struct Context* context = nullptr;		// NanoVG context
	GrInputMap input;						// Input map for storing user input

public:
	int width;								// Width of the window
	int height;								// Height of the window

	// Constructor
	OdGrUiWindow(int aWidth, int aHeight, const char* title)
	{
		width = aWidth;
		height = aHeight;
		input = GrInputMap();

		// Initialize GLFW
		if (!glfwInit())
			return;

		// Create window with graphics context
		glfwHandle = glfwCreateWindow(width, height, title, nullptr, nullptr);
		if (glfwHandle == nullptr)
			return;

		glfwMakeContextCurrent(glfwHandle);		// Set context as current
		glfwSwapInterval(1);					// Enable vsync
		glewInit();								// Initialize glew

		// Initialize NanoVG context (OpenGL backend)
		context = nvgCreateGL3(NVG_ANTIALIAS | NVG_STENCIL_STROKES | NVG_DEBUG);
		if (context == nullptr) {
			return;
		}

		// Bind this instance of GrWindow to the glfw window instance for static callbacks
		glfwSetWindowUserPointer(glfwHandle, this);

		// Set the callback function for mouse movement
		glfwSetCursorPosCallback(glfwHandle, mousePositionEventCallback);
		glfwSetMouseButtonCallback(glfwHandle, mouseClickEventCallback);
		glfwSetKeyCallback(glfwHandle, keyEventCallback);
	}

	// Callback function for mouse movement
	static void mousePositionEventCallback(GLFWwindow* window, double xpos, double ypos)
	{
		OdGrUiWindow* windowInstance = static_cast<OdGrUiWindow*>(glfwGetWindowUserPointer(window));
		if (windowInstance) {
			// Access the instance and store the mouse position
			windowInstance->input.mouse.position.x = xpos;
			windowInstance->input.mouse.position.y = ypos;
		}
	}

	// Callback function for mouse clicks
	static void mouseClickEventCallback(GLFWwindow* window, int button, int action, int mods)
	{
		OdGrUiWindow* windowInstance = static_cast<OdGrUiWindow*>(glfwGetWindowUserPointer(window));
		if (windowInstance)
		{
			switch (button)
			{
			case 0:
				if (action == GLFW_PRESS)
					windowInstance->input.mouse.leftButton.changeState(true);
				if (action == GLFW_RELEASE)
					windowInstance->input.mouse.leftButton.changeState(false);
				break;

			case 1:
				if (action == GLFW_PRESS)
					windowInstance->input.mouse.rightButton.changeState(true);
				if (action == GLFW_RELEASE)
					windowInstance->input.mouse.rightButton.changeState(false);
				break;

			case 2:
				if (action == GLFW_PRESS)
					windowInstance->input.mouse.middleButton.changeState(true);
				if (action == GLFW_RELEASE)
					windowInstance->input.mouse.middleButton.changeState(false);
				break;
			}
		}
	}

	// Callback function for key events
	static void keyEventCallback(GLFWwindow* window, int key, int scancode, int action, int mods)
	{
		OdGrUiWindow* windowInstance = static_cast<OdGrUiWindow*>(glfwGetWindowUserPointer(window));
		if (windowInstance) {
			GrInputMap* map = &windowInstance->input;

			if (map->keys.find(key) == map->keys.end())
				return;

			// Change button state to down
			if (action == GLFW_PRESS)
				map->keys[key].changeState(true);

			// Change button state to up
			if (action == GLFW_RELEASE)
				map->keys[key].changeState(false);

		}
	}

	// Check if the window is still running
	bool isRunning()
	{
		return !glfwWindowShouldClose(glfwHandle);
	}

	// Set the current context
	void makeCurrentContext()
	{
		glfwMakeContextCurrent(glfwHandle);
	}

	// Get the NanoVG context
	NVGcontext* getContext()
	{
		return context;
	}

	OdSyPoint getLocationInContext()
	{
		return OdSyPoint(0, 0);
	}

	// Close the window and clean up resources
	void close()
	{
		glfwDestroyWindow(glfwHandle);
		glfwTerminate();

		// Delete UI components
		for (OdGrUiComponent* control : childComponents) {
			delete control;
		}
	}

	// Initialize the window and UI components
	virtual void initialise()
	{

	}

	// Render the window and UI components
	virtual void const onFrame(NVGcontext* context)
	{
		glfwPollEvents();
	}

	// Process events for UI components
	void triggerEventsChain()
	{
		for (OdGrUiComponent* control : childComponents)
		{
			control->processEvents(&input);
		}

		// Once the current input states are filtered through all the components
		// reset the 'pressed'/'released' states to prevent double trigger
		if (input.mouse.leftButton.isDown())
			input.mouse.leftButton.changeState(true);

		if (input.mouse.middleButton.isDown())
			input.mouse.middleButton.changeState(true);

		if (input.mouse.rightButton.isDown())
			input.mouse.rightButton.changeState(true);
	}

};
