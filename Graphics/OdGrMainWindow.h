#include "Graphics/OdGrGraphics.h"

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyColour.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 14, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for the main application window                                           *
***************************************************************************************/



class OdGrMainWindow : public OdGrUiWindow
{
public:

	OdSyImage* img;

	OdGrMainWindow(int aWidth, int aHeight, const char* title) : OdGrUiWindow(aWidth, aHeight, title) {
		OdGrDraw::LoadImage(context, 256, 256, ".\\testimage.jpg", &img);
	}

	~OdGrMainWindow()
	{
		delete img;

		// Update UI components
		for (OdGrUiComponent* control : childComponents) {
			delete control;
		}
	}


	// Initialize the window and UI components
	void initialise() override
	{
		OdGrDraw::LoadFont(context);

		OdGrUiButton* btn1 = new OdGrUiButton(300, 300);
		btn1->setText("Button 1");
		addChildControl(btn1);

		OdGrUiButton* btn2 = new OdGrUiButton(300, 50);
		btn2->setText("Button 2");
		addChildControl(btn2);

	}

	// Render the window and UI components
	virtual void const onFrame(NVGcontext* vg = nullptr, OdGrUiComponent* parent = nullptr)
	{
		glClearColor(0.3f, 0.3f, 0.32f, 1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

		nvgBeginFrame(context, 1280, 720, 1);

		if (childComponents[0]->isMouseDown())
			childComponents[1]->setText("HELLO WORLD");

		
		OdGrDraw::ImageFromPath(context, 700, 200, img);

		// Update UI components
		for (OdGrUiComponent* control : childComponents) {
			control->onFrame(context);
		}


		nvgEndFrame(context);

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}
};