#ifndef OD_GR_MAIN_WINDOW_CPP
#define OD_GR_MAIN_WINDOW_CPP

/**************************************************************************************
* OpenDraft:    Main Application Window Class                                         *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         July 08, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class for the main application window                                           *
***************************************************************************************/



#include <chrono>
#include "Graphics/OdMainWindow.h"
#include "System/OdApplication.h"
#include "DataManager/OdData.h"
#include "System/OdEditor.h"


namespace OD::Graphics
{

	// Load required resources
	void OdMainWindow::loadResources()
	{
		resourceManager->importFontFromFile(context, "sans", "..\\Resources\\Fonts\\OpenSans.ttf");

		std::vector<std::string> icons = {
			"new_file", "open", "save", "save_as", "print", "undo", "redo",
			"line", "polyline", "rectangle", "circle", "text",
			"move", "rotate", "mirror", "array"
		};

		for (int i = 0; i < icons.size(); i++)
		{
			std::string name = "ico_" + icons[i];
			std::string path = "..\\Resources\\Icons\\" + icons[i] + ".png";
			resourceManager->importImageFromFile(context, 32, 32, name.c_str(), path.c_str());
		}

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


		//
		// Start Ribbon Area
		//

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

		int BUTTON_SIZE = 34;
		int BUTTON_MARGIN = 2;



		// Home Tab
		std::vector<std::string> homeButtons = { "new_file", "open", "save", "save_as", "print", "undo", "redo" };

		for (int i = 0; i < homeButtons.size(); i++)
		{
			OdButton* btn = new OdButton(10 + (BUTTON_SIZE + BUTTON_MARGIN) * i, 10, BUTTON_SIZE, BUTTON_SIZE, "");
			btn->backColour = OdColour::CLEAR;
			btn->backColourHover = OdColour::BACKGROUND1;
			btn->setBackgroundImage(resourceManager->images["ico_" + homeButtons[i]]);
			tabHome->addPanelChildControl(btn);
		}



		// Draw Tab
		std::vector<std::string> drawButtons = { "line", "polyline", "rectangle", "circle", "text"};

		for (int i = 0; i < drawButtons.size(); i++)
		{
			OdButton* btn = new OdButton(10 + (BUTTON_SIZE + BUTTON_MARGIN) * i, 10, BUTTON_SIZE, BUTTON_SIZE, "");
			btn->backColour = OdColour::CLEAR;
			btn->backColourHover = OdColour::BACKGROUND1;
			btn->setBackgroundImage(resourceManager->images["ico_" + drawButtons[i]]);
			tabDraw->addPanelChildControl(btn);

		}


		// Modify Tab
		std::vector<std::string> modifyButtons = { "move", "rotate", "mirror", "array"};

		for (int i = 0; i < modifyButtons.size(); i++)
		{
			OdButton* btn = new OdButton(10 + (BUTTON_SIZE + BUTTON_MARGIN) * i, 10, BUTTON_SIZE, BUTTON_SIZE, "");
			btn->backColour = OdColour::CLEAR;
			btn->backColourHover = OdColour::BACKGROUND1;
			btn->setBackgroundImage(resourceManager->images["ico_" + modifyButtons[i]]);
			tabModify->addPanelChildControl(btn);
		}

		// End Ribbon Area

		
		// Set tab 1 as active
		tabV->setActiveTab(0);

		// Setup the viewport
		viewport = new OdViewport(0, 100, 1280, 700);
		viewport->setAnchor(OdAnchor::Direction::Left, true);
		viewport->setAnchor(OdAnchor::Direction::Right, true);
		viewport->setAnchor(OdAnchor::Direction::Top, true);
		viewport->setAnchor(OdAnchor::Direction::Bottom, true);
		viewport->backColour = OdColour(0, 0, 0, 50);
		addChildControl(viewport);



		// DEBUG & TESTING

		//OdLabel* lbl1 = new OdLabel(100, 600, 150, 50, "Hello World");
		//OdLabel* lbl1 = new OdLabel(100, 600, 150, 50, "Hello World\nHello World 2");
		OdLabel* lbl1 = new OdLabel(100, 600, 150, 50, "Hello World\nHello World 2\nHello World 3\nHello World 4");
		lbl1->backColour = OdColour::RED;
		lbl1->setSingleLine(false);
		lbl1->setAlign(OdAlign(OdAlign::LEFT | OdAlign::MIDDLE));
		lbl1->setTextWrap(true);
		lbl1->zOrder = 200;
		addChildControl(lbl1);

		// !DEBUG & TESTING


		//
		// 
		// 
		// Testing
		//
		//
		//

		/*OdLabel* lbl1 = new OdLabel(750, 150, 500, 20, "Hello World");
		lbl1->backColour = OdColour::RED;
		lbl1->zOrder = 200;
		addChildControl(lbl1);*/

		OdApplication* app = OdApplication::getInstance();
		OdDocumentManager* docManager = app->getDocumentManager();

		OdDocument* doc1 = new OdDocument();
		docManager->addDocument(doc1);
		docManager->setActiveDocument(0);

		OdDocument* doc = docManager->getActiveDocument();
		OdDrawingDb* db = doc->getDatabase();
		OdEditor* ed = doc->getEditor();


		doc->author = L"James Hodgkins";
		doc->comments = L"This is a test document comment";
		doc->company = L"OpenDraft";
		

		OdEllipse* ellipse1 = new OdEllipse(250, 250, 50, 20);
		ellipse1->rotation = 45;
		ellipse1->setColour(2);
		db->addCreatedEntity(ellipse1);
		ed->addEntityToSelection(ellipse1);
		/*
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

		OdArc *arc1 = new OdArc(200, 100, 40, 0, 90);
		arc1->setColour(5);
		db->addCreatedEntity(arc1);

		

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
	void OdMainWindow::onFrame(NVGcontext* NULLREF)
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
		/*OdViewport* vp = static_cast<OdViewport*>(childComponents[1].get());
		
		float mouseX = input.mouse.position.x;
		float mouseY = input.mouse.position.y;
		float scaleF = vp->getScale();

		OdVector2 cursorPos = vp->getCoordinatesAtScreenPosition(mouseX, mouseY);
		OdVector2 screenPos = vp->getScreenPositionAtCoordinates(cursorPos.x, cursorPos.y);

		newText += "Mouse Position: " + std::to_string(screenPos.x) + ", " + std::to_string(screenPos.y)
			+ " / " + std::to_string(mouseX) + ", " + std::to_string(mouseY) + ")" + "\n";

		childComponents[2]->text = newText;*/


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

#endif // !OD_GR_MAIN_WINDOW_CPP