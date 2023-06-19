#include "Graphics/OdGrGraphics.h"

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyColour.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 19, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for the main application window                                           *
***************************************************************************************/



class OdGrMainWindow : public OdGrUiWindow
{
public:

	OdGrMainWindow(int aWidth, int aHeight, const char* aTitle) : OdGrUiWindow(aWidth, aHeight, aTitle) {}

	~OdGrMainWindow(){}


	// Initialize the window and UI components
	void initialise() override
	{
		loadResources();

		backColour.setRGB(31,39,48);

		OdGrUiButton* btn1 = new OdGrUiButton(300, 300);
		btn1->setText("Show");
		addChildControl(btn1);

		OdGrUiButton* btn2 = new OdGrUiButton(300, 230);
		btn2->setText("Hide");
		addChildControl(btn2);


		// Start Ribbon Area
		OdGrUiPanel* pnl1 = new OdGrUiPanel(200, 0, 1000, 100);
		pnl1->backColour = OdSyColour::BACKGROUND2;
		addChildControl(pnl1);

		OdGrUiButton* btn3 = new OdGrUiButton(10, 10, 24, 24, "");
		btn3->backColour = OdSyColour::TRANSP;
		btn3->setBackgroundImage(resourceManager->images["ico_circle"]);
		pnl1->addChildControl(btn3);

		OdGrUiButton* btn4 = new OdGrUiButton(55, 10, 24, 24, "");
		btn4->backColour = OdSyColour::TRANSP;
		btn4->setBackgroundImage(resourceManager->images["ico_line"]);
		pnl1->addChildControl(btn4);

		OdGrUiButton* btn5 = new OdGrUiButton(100, 10, 24, 24, "");
		btn5->backColour = OdSyColour::TRANSP;
		btn5->setBackgroundImage(resourceManager->images["ico_polyline"]);
		pnl1->addChildControl(btn5);

	}

	// Load required resources
	void loadResources()
	{
		resourceManager->importFontFromFile(context, "sans", "..\\Resources\\Fonts\\OpenSans.ttf");

		// load 32x32 png icons
		resourceManager->importImageFromFile(context, 32, 32, "ico_circle", "..\\Resources\\Icons\\circle.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_line", "..\\Resources\\Icons\\line.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_mirror", "..\\Resources\\Icons\\mirror.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_newfile", "..\\Resources\\Icons\\new_file.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_open", "..\\Resources\\Icons\\open.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_polyline", "..\\Resources\\Icons\\polyline.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_print", "..\\Resources\\Icons\\print.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_rectangle", "..\\Resources\\Icons\\rectangle.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_redo", "..\\Resources\\Icons\\redo.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_save", "..\\Resources\\Icons\\save.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_saveas", "..\\Resources\\Icons\\save_as.png");
		resourceManager->importImageFromFile(context, 32, 32, "ico_undo", "..\\Resources\\Icons\\undo.png");

	}

	// Render the window and UI components
	virtual void const onFrame(NVGcontext* NULLREF = nullptr, OdGrUiComponent* aParent = nullptr)
	{
		glClearColor(
			OdSyColour::BACKGROUND1.getRedNorm(),
			OdSyColour::BACKGROUND1.getGreenNorm(),
			OdSyColour::BACKGROUND1.getBlueNorm(),
			1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

		glViewport(0, 0, size.x, size.y);
		nvgBeginFrame(context, size.x, size.y, 1);



		// Test updates on state
		if (childComponents[0]->isMouseDown())
			childComponents[2]->enabled = true;

		// Test updates on state
		if (childComponents[1]->isMouseDown())
			childComponents[2]->enabled = false;


		
		// Update UI components
		for (OdGrUiComponent* control : childComponents) {
			control->onFrame(context);
		}


		nvgEndFrame(context);

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}
};