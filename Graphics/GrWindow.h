#pragma once

#include <vector>
#include <iostream>

#define GLEW_STATIC					// Use static glew library (glew32s.lib)
#include "GL/glew.h"				// Include glew
#include "glfw/glfw3.h"				// Include glfw3

#define NANOVG_GL3_IMPLEMENTATION	// Use GL3.
#include "nanovg.h"					// Include core nanovg library
#include "nanovg_gl.h"				// Include nanovg opengl3 implementation

#define Window GLFWwindow			// Alias Window for GLFWwindow
#define Context NVGcontext			// Alias Window for GLFWwindow

#include "Graphics/UI/GrInput.h"
#include "Graphics/UI/GrUIComponents.h"

// Class representing a window
class GrWindow
{
private:
	GLFWwindow* glfwHandle = nullptr;		// Handle to the GLFW window
	struct Context* context = nullptr;		// NanoVG context
	GrInputMap input;						// Input map for storing user input

	std::vector<GrUIComponent*> controls;	// Vector to store UI components

public:
	int width;								// Width of the window
	int height;								// Height of the window

	// Constructor
	GrWindow(int aWidth, int aHeight, const char* title)
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
		GrWindow* windowInstance = static_cast<GrWindow*>(glfwGetWindowUserPointer(window));
		if (windowInstance) {
			// Access the instance and store the mouse position
			windowInstance->input.mouse.position.x = xpos;
			windowInstance->input.mouse.position.y = ypos;
		}
	}

	// Callback function for mouse clicks
	static void mouseClickEventCallback(GLFWwindow* window, int button, int action, int mods)
	{
		GrWindow* windowInstance = static_cast<GrWindow*>(glfwGetWindowUserPointer(window));
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
		GrWindow* windowInstance = static_cast<GrWindow*>(glfwGetWindowUserPointer(window));
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

	// Close the window and clean up resources
	void close()
	{
		glfwDestroyWindow(glfwHandle);
		glfwTerminate();

		// Delete UI components
		for (GrUIComponent* control : controls) {
			delete control;
		}
	}

	// Initialize the window and UI components
	void initialise()
	{
		GrUIButton* btn = new GrUIButton(300, 300);
		controls.push_back(btn);

		GrUIButton* btn2 = new GrUIButton(300, 50);
		controls.push_back(btn2);
	}

	// Render the window and UI components
	void render()
	{
		glClearColor(0.3f, 0.3f, 0.32f, 1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

		nvgBeginFrame(context, 1280, 720, 1);

		// Render UI components
		for (GrUIComponent* control : controls) {
			control->render(context);
		}

		nvgEndFrame(context);

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}

	// Process events for UI components
	void startEventsChain()
	{
		for (GrUIComponent* control : controls)
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
