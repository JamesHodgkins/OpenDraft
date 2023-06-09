#include "Graphics/UI/GrUIWindow.h"

class GrMainWindow : public GrUIWindow
{
public:

	GrMainWindow(int aWidth, int aHeight, const char* title) : GrUIWindow(aWidth, aHeight, title) {}


	// Initialize the window and UI components
	void initialise() override
	{
		GrUIButton* btn = new GrUIButton(300, 300);
		controls.push_back(btn);

		GrUIButton* btn2 = new GrUIButton(300, 50);
		controls.push_back(btn2);
	}

	// Render the window and UI components
	void render() override
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
};