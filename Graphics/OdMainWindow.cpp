#ifndef OD_GR_MAIN_WINDOW_CPP
#define OD_GR_MAIN_WINDOW_CPP

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    �2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for the main application window                                           *
***************************************************************************************/



#include "Graphics/OdMainWindow.h"
#include "System/OdApplication.h"


namespace OD::Graphics
{

	// Load required resources
	void OdMainWindow::loadResources()
	{
		resourceManager->importFontFromFile(context, "sans", "..\\Resources\\Fonts\\OpenSans.ttf");

		// Load 32x32 GUI PNG icons
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


	OdMainWindow::OdMainWindow(int aWidth, int aHeight, const char* aTitle) : OdWindow(aWidth, aHeight, aTitle) {}

	OdMainWindow::~OdMainWindow() {}


	// Initialize the window and UI components
	void OdMainWindow::initialise()
	{
		

		loadResources();

		backColour.setRGB(31, 39, 48);


		OdButton* btn1 = new OdButton(200, 110);
		btn1->setText("Show");
		addChildControl(btn1);
		btn1->setAnchor(OdAnchor::Direction::Left, true);

		OdButton* btn2 = new OdButton(400, 210);
		btn2->setText("Hide");
		addChildControl(btn2);
		btn2->setAnchor(OdAnchor::Direction::Left, true);


		// Start Ribbon Area
		OdPanel* pnl1 = new OdPanel(0, 400, size.x, 100);
		pnl1->backColour = OdColour::BACKGROUND2;
		addChildControl(pnl1);

		OdButton* btn3 = new OdButton(10, 10, 24, 24, "");
		btn3->backColour = OdColour::CLEAR;
		btn3->setBackgroundImage(resourceManager->images["ico_circle"]);
		pnl1->addChildControl(btn3);

		OdButton* btn4 = new OdButton(55, 10, 24, 24, "");
		btn4->backColour = OdColour::CLEAR;
		btn4->setBackgroundImage(resourceManager->images["ico_line"]);
		pnl1->addChildControl(btn4);

		OdButton* btn5 = new OdButton(100, 10, 24, 24, "");
		btn5->backColour = OdColour::CLEAR;
		btn5->setBackgroundImage(resourceManager->images["ico_polyline"]);
		pnl1->addChildControl(btn5);

		OdTabView* tabV = new OdTabView(0, 0, 1280, 100);
		addChildControl(tabV);

		tabV->addTab("Home");
		tabV->addTab("Modify");
		tabV->addTab("View");
		tabV->addTab("Help");

		OdTab* t1 = tabV->getTab(0);
		OdTab* t2 = tabV->getTab(1);
		OdTab* t3 = tabV->getTab(2);
		OdTab* t4 = tabV->getTab(3);

		OdLabel* lb1 = new OdLabel(10, 10, 100, 20, "Hello World");

		t1->addPanelChildControl(lb1);

		// Set tab 1 as active
		tabV->setActiveTab(0);
	}


	// Render the window and UI components
	void OdMainWindow::onFrame(NVGcontext* NULLREF, OdComponent* aParent)
	{
		OdApplication* app = OdApplication::getInstance();
		OdWindow* wnd = app->getMainWindow();

		glClearColor(
			OdColour::BACKGROUND1.getRedNorm(),
			OdColour::BACKGROUND1.getGreenNorm(),
			OdColour::BACKGROUND1.getBlueNorm(),
			1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);

		// Cast window size
		float w = static_cast<float>(size.x);
		float h = static_cast<float>(size.y);

		glViewport(0, 0, w, h);

		nvgBeginFrame(context, w, h, 1);



		//// Test updates on state
		//if (childComponents[0]->isMouseDown())
		//	childComponents[2]->enabled = true;

		//// Test updates on state
		//if (childComponents[1]->isMouseDown())
		//	childComponents[2]->enabled = false;



		// Draw child UI components
		drawChildComponents(context);


		nvgEndFrame(context);

		closeEvents();

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}

}// namespace OD::Graphics

#endif // OD_GR_MAIN_WINDOW_CPP