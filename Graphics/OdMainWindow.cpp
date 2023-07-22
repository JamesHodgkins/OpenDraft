#ifndef OD_GR_MAIN_WINDOW_CPP
#define OD_GR_MAIN_WINDOW_CPP

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         July 08, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for the main application window                                           *
***************************************************************************************/



#include <chrono>
#include "Graphics/OdMainWindow.h"
#include "System/OdApplication.h"
#include "DataManager/OdData.h"


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

	OdMainWindow::~OdMainWindow() = default;


	// Initialize the window and UI components
	void OdMainWindow::initialise()
	{
		loadResources();

		backColour.setRGB(31, 39, 48);


		/*OdButton* btn1 = new OdButton(200, 110);
		btn1->setText("Show");
		addChildControl(btn1);
		btn1->setAnchor(OdAnchor::Direction::Left, true);

		OdButton* btn2 = new OdButton(400, 210);
		btn2->setText("Hide");
		addChildControl(btn2);
		btn2->setAnchor(OdAnchor::Direction::Left, true);*/


		// Start Ribbon Area
		/*OdPanel* pnl1 = new OdPanel(0, 400, size.x, 100);
		pnl1->backColour = OdColour::BACKGROUND2;
		addChildControl(pnl1);	*/

		OdTabView* tabV = new OdTabView(0, 0, 1280, 100);
		tabV->setAnchor(OdAnchor::Direction::Left, true);
		tabV->setAnchor(OdAnchor::Direction::Right, true);
		addChildControl(tabV);

		tabV->addTab("Home");
		tabV->addTab("Draw");
		tabV->addTab("Modify");
		tabV->addTab("View");
		tabV->addTab("Help");

		OdTab* tabHome = tabV->getTab(0);
		OdTab* tabDraw = tabV->getTab(1);
		OdTab* tabModify = tabV->getTab(2);
		OdTab* tabView = tabV->getTab(3);
		OdTab* tabHelp = tabV->getTab(4);

		OdButton* btn3 = new OdButton(10, 10, 34, 34, "");
		btn3->backColour = OdColour::CLEAR;
		btn3->backColourHover = OdColour::BACKGROUND1;
		btn3->setBackgroundImage(resourceManager->images["ico_circle"]);
		tabDraw->addPanelChildControl(btn3);

		OdButton* btn4 = new OdButton(46, 10, 34, 34, "");
		btn4->backColour = OdColour::CLEAR;
		btn4->backColourHover = OdColour::BACKGROUND1;
		btn4->setBackgroundImage(resourceManager->images["ico_line"]);
		tabDraw->addPanelChildControl(btn4);

		OdButton* btn5 = new OdButton(82, 10, 34, 34, "");
		btn5->backColour = OdColour::CLEAR;
		btn5->backColourHover = OdColour::BACKGROUND1;
		btn5->setBackgroundImage(resourceManager->images["ico_polyline"]);
		tabDraw->addPanelChildControl(btn5);

		// Set tab 1 as active
		tabV->setActiveTab(0);


		OdLabel* lbl1 = new OdLabel(800, 7, 100, 20, "Hello World");
		addChildControl(lbl1);


		//
		// 
		// 
		// Testing
		//
		//
		//

		OdApplication* app = OdApplication::getInstance();
		OdDocumentManager* docManager = app->getDocumentManager();

		OdDocument* doc1 = new OdDocument();
		docManager->addDocument(doc1);
		docManager->setActiveDocument(0);

		OdDocument* doc = docManager->getActiveDocument();
		OdDrawingDb* db = doc->getDatabase();


		doc->author = L"James Hodgkins";
		doc->comments = L"This is a test document comment";
		doc->company = L"OpenDraft";
		
		viewport = new OdViewport(0, 100, 1280, 700);
		viewport->setAnchor(OdAnchor::Direction::Left, true);
		viewport->setAnchor(OdAnchor::Direction::Right, true);
		viewport->setAnchor(OdAnchor::Direction::Top, true);
		viewport->setAnchor(OdAnchor::Direction::Bottom, true);
		viewport->backColour = OdColour(0,0,0,50);
		addChildControl(viewport);
		
		/*OdEllipse *ellipse1 = new OdEllipse(250, 250, 50, 20);
		ellipse1->rotation = 45;
		ellipse1->setColour(2);
		db->addCreatedEntity(ellipse1);

		OdEllipse* ellipse2 = new OdEllipse(50, 50, 50, 20);
		ellipse2->rotation = -45;
		ellipse2->setColour(10);
		db->addCreatedEntity(ellipse2);*/
		
		OdLine* line1 = new OdLine(100, 100, 250, 250);
		line1->setColour(3);
		line1->setLineWeight(3);
		db->addCreatedEntity(line1);

		OdLine* line2 = new OdLine(250, 50, 50, 250);
		line2->setColour(10);
		db->addCreatedEntity(line2);

		OdCircle* circle1 = new OdCircle(100, 100, 50);
		circle1->setColour(4);
		db->addCreatedEntity(circle1);


		

		// Get current date as unix timestamp
		std::chrono::system_clock::time_point now = std::chrono::system_clock::now();

		// Convert the system time point to a duration since epoch
		std::chrono::seconds duration = std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch());

		// Extract the count of seconds as an int value
		int timestamp = static_cast<int>(duration.count());

		doc->creationDate = timestamp;
		doc->modifiedDate = timestamp;
		doc->updateDocument();

		
	}


	// Render the window and UI components
	void OdMainWindow::onFrame(NVGcontext* NULLREF, OdComponent* aParent)
	{
		nvgReset(context);

		OdApplication* app = OdApplication::getInstance();

		glClearColor(
			OdColour::BACKGROUND1.getRedNorm(),
			OdColour::BACKGROUND1.getGreenNorm(),
			OdColour::BACKGROUND1.getBlueNorm(),
			1.0f);
		glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);


		// Hide cursor when over viewport
		if (viewport->isMouseOver())
			glfwSetInputMode(glfwHandle, GLFW_CURSOR, GLFW_CURSOR_HIDDEN);
		else
			glfwSetInputMode(glfwHandle, GLFW_CURSOR, GLFW_CURSOR_NORMAL);
		

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

		std::string newText = "";

		// Cast childComponent[2] as OdViewport
		OdViewport* vp = static_cast<OdViewport*>(childComponents[2].get());
		
		float mouseX = input.mouse.position.x;
		float mouseY = input.mouse.position.y;
		float scaleF = vp->getScale();
		OdVector2 pos = vp->getPosition();

		OdVector2 cursorPos = vp->getCoordinatesAtScreenPosition(mouseX, mouseY);

		newText += "Mouse Position: " + std::to_string(cursorPos.x) + ", " + std::to_string(cursorPos.y)
			+ ", " + std::to_string(scaleF) + " " + std::to_string(pos.x) + ", " + std::to_string(pos.y) + ")" + "\n";

		childComponents[1]->text = newText;


		// Draw child UI components
		drawChildComponents(context);


		nvgEndFrame(context);

		closeEvents();

		glfwPollEvents();
		glfwSwapBuffers(glfwHandle);
	}


	// Action main window events
	void OdMainWindow::actionEvents(GrInputMap* aInput)
	{

	}

}// namespace OD::Graphics

#endif // OD_GR_MAIN_WINDOW_CPP