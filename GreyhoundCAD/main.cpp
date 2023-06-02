#include "glfw/glfw3.h"


// Main code
int main(int, char**)
{
	//glfwSetErrorCallback(glfw_error_callback);
	if (!glfwInit())
		return 1;

	// Create window with graphics context
	GLFWwindow* window = glfwCreateWindow(1280, 720, "GreyhoundCAD", nullptr, nullptr);
	if (window == nullptr)
		return 1;

	glfwMakeContextCurrent(window);
	glfwSwapInterval(1); // Enable vsync

	// Main loop
	while (!glfwWindowShouldClose(window))
	{
		glfwPollEvents();
		glfwSwapBuffers(window);
	}

	glfwDestroyWindow(window);
	glfwTerminate();

	return 0;
}