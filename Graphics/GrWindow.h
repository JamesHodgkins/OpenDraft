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


class GrWindow
{
private:
	GLFWwindow* glfwHandle = nullptr;
	struct Context* context = nullptr;		// Declare the context
	GrInputMap input;

	std::vector<GrUIComponent*>controls;
	

public:
	int width;
	int height;


	GrWindow(int aWidth, int aHeight, const char* title)
	{
		width = aWidth;
		height = aHeight;
		input = GrInputMap();

		// glfwSetErrorCallback(glfw_error_callback);
		if (!glfwInit())
			return;

		
		// Create window with graphics context: glfwWindowHint(GLFW_DECORATED, GLFW_FALSE);
		glfwHandle = glfwCreateWindow(width, height, title, nullptr, nullptr);

		if (glfwHandle == nullptr)
			return;

		glfwMakeContextCurrent(glfwHandle);		// Set context as current
		glfwSwapInterval(1);					// Enable vsync
		glewInit();								// Initialise glew


		// Initialize NanoVG context (OpenGL backend)
		context = nvgCreateGL3(NVG_ANTIALIAS | NVG_STENCIL_STROKES | NVG_DEBUG);
		if (context == nullptr) {
			return;
		}


		// Bind this instance of GrWindow to the glfw window instance for static callbacks
		glfwSetWindowUserPointer(glfwHandle, this);

		// Set the callback function for mouse movement
		glfwSetCursorPosCallback(glfwHandle, mousePositionEventCallback);
		glfwSetKeyCallback(glfwHandle, keyEventCallback);
	}


	//
	// Bind callbacks to update window states
	//
	static void mousePositionEventCallback(GLFWwindow* window, double xpos, double ypos)
	{
		GrWindow* windowInstance = static_cast<GrWindow*>(glfwGetWindowUserPointer(window));
		if (windowInstance) {
			// Access the instance and store the value
			windowInstance->input.mouse.position.x = xpos;
			windowInstance->input.mouse.position.y = ypos;
		}
	}

	static void keyEventCallback(GLFWwindow* window, int key, int scancode, int action, int mods)
	{
		GrWindow* windowInstance = static_cast<GrWindow*>(glfwGetWindowUserPointer(window));
		if (windowInstance) {

			GrInputMap* map = &windowInstance->input;

			if (map->keys.find(key) == map->keys.end()) {
				std::cout << "KEY NOT MAPPED" << std::endl;
				return;
			}

			if (action == GLFW_PRESS)
				map->keys[key].changeState(true);

			if (action == GLFW_RELEASE)
				map->keys[key].changeState(false);
		
			if (map->keys[key].isPressed())
			{
				std::cout << "Key " << key << " is pressed"  << std::endl;
			}
			else if (map->keys[key].isReleased())
			{
				std::cout << "Key " << key << " is released" << std::endl;
			}
			
		}
	}



	bool isRunning()
	{
		return !glfwWindowShouldClose(glfwHandle);
	}

	void makeCurrentContext()
	{
		glfwMakeContextCurrent(glfwHandle);		// Set context as current
	}

	NVGcontext* getContext()
	{
		return context;
	}

	void close()
	{
		glfwDestroyWindow(glfwHandle);
		glfwTerminate();

		for (GrUIComponent* control : controls) {
			delete control;
		}
	}

	void initialise()
	{
		GrUIButton* btn = new GrUIButton(300,300);
		controls.push_back(btn);

		GrUIButton* btn2 = new GrUIButton(300, 50);
		controls.push_back(btn2);
	}

	void render()
	{
		glClearColor(0.3f, 0.3f, 0.32f, 1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

		nvgBeginFrame(context, 1280, 720, 1);

		for (GrUIComponent* control : controls) {
			control->render(context);
		}

		nvgEndFrame(context);

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}


	void startEventsChain()
	{
		for (GrUIComponent* control : controls)
		{
			control->processEvents(&input);
		}
	}

};