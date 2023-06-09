#include "Graphics/UI/OdGrUiWindow.h"

class OdGrMainWindow : public OdGrUiWindow
{
public:

	OdGrMainWindow(int aWidth, int aHeight, const char* title) : OdGrUiWindow(aWidth, aHeight, title) {}


	// Initialize the window and UI components
	void initialise() override
	{
		OdGrUiButton* btn1 = new OdGrUiButton(300, 300);
		btn1->setText("Button 1");
		controls.push_back(btn1);

		OdGrUiButton* btn2 = new OdGrUiButton(300, 50);
		btn2->setText("Button 2");
		controls.push_back(btn2);
	}

	// Render the window and UI components
	void startFrameProcess() override
	{
		glClearColor(0.3f, 0.3f, 0.32f, 1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

		nvgBeginFrame(context, 1280, 720, 1);

		if (controls[0]->isMouseDown())
			controls[1]->setText("HELLO WORLD");

		// Update UI components
		for (OdGrUiComponent* control : controls) {
			control->onFrame(context);
		}

		nvgEndFrame(context);

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}
};