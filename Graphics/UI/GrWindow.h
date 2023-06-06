#pragma once

#define GLEW_STATIC					// Use static glew library (glew32s.lib)
#include "GL/glew.h"				// Include glew
#include "glfw/glfw3.h"				// Include glfw3

#define NANOVG_GL3_IMPLEMENTATION	// Use GL3.
#include "nanovg.h"					// Include core nanovg library
#include "nanovg_gl.h"				// Include nanovg opengl3 implementation

#define Window GLFWwindow			// Alias Window for GLFWwindow
#define Context NVGcontext			// Alias Window for GLFWwindow

class GrWindow
{
private:
	GLFWwindow* glfwHandle = nullptr;
	struct Context* context = nullptr;		// Declare the context

public:
	int width;
	int height;


	GrWindow(int aWidth, int aHeight, const char* title)
	{
		width = aWidth;
		height = aHeight;

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
		if (context == NULL) {
			return;
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

	void endFrame()
	{
		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}

	NVGcontext* getContext()
	{
		return context;
	}

	void close()
	{
		glfwDestroyWindow(glfwHandle);
		glfwTerminate();
	}
};