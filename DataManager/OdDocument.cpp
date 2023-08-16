#ifndef OD_DAT_DOCUMENT_CPP
#define OD_DAT_DOCUMENT_CPP

/**************************************************************************************
* OpenDraft:    Document Class                                                        *
*-------------------------------------------------------------------------------------*
* Filename:     OdDocument.cpp                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include <cstring>
#include "DataManager/OdDocument.h"
#include "System/OdCore.h"				// Include Core Utilities
#include "System/Objects/OdLayer.h"		// Include Layer Object
#include "System/OdEditor.h"			// Include Editor


namespace OD::Data
{
	// Constructors and Destructors
	OdDocument::OdDocument() : version{'O','D','0','0','\0'}
	{
		editor = new System::OdEditor();
		path = "";
		readOnly = false;
		modified = false;
		locked = false;

		// Create active layer and set to 'Undefined' default layer
		activeLayer = new OdLayer("Undefined","Undefined default layer", 7, "", 0);
		database.addRecord(dynamic_cast<OdDbObject*>(activeLayer));
	}
	
	OdDocument::~OdDocument()
	{

	}


	// 
	// Getters and Setters
	//

	// Document Path
	std::string OdDocument::getPath()
	{
		return path;
	}
	
	void OdDocument::setPath(std::string aPath)
	{
		path = aPath;
	}

	// Document Version
	std::string OdDocument::getVersion()
	{
		return version;
	}
	void OdDocument::setVersion(const char* aVersion)
	{
		strncpy_s(version, aVersion, sizeof(version));
		version[sizeof(version) - 1] = '\0'; // Ensure null-termination
	}

	// Document Read-Only
	bool OdDocument::isReadOnly()
	{
		return readOnly;
	}
	
	// Upgrade open status to read-write
	bool OdDocument::upgradeOpen()
	{
		if (locked)
		{
			return false;
		}
		else
		{
			readOnly = false;
			return true;
		}
	}

	// Document Modified
	bool OdDocument::isModified()
	{
		return modified;
	}
	
	void OdDocument::setModified(bool aModified)
	{
		modified = aModified;
	}

	// Document Locked
	bool OdDocument::isLocked()
	{
		return locked;
	}
	void OdDocument::setLocked(bool aLocked)
	{
		locked = aLocked;
	}

	// Document Save
	bool OdDocument::save()
	{
		// Check if document is read-only or locked
		if (readOnly || locked)
			return false;

		
		// Check if file does not already exists
		if (path == "")
			createNew("");

		// Save file
		updateDocument();

		return true;
	}

	// Document Save As
	bool OdDocument::saveAs(std::string aPath)
	{
		return true;
	}

	// Create new Document
	bool OdDocument::createNew(std::string aPath)
	{
		// if path is empty, ask user for path
		if (aPath == "")
		{
			// TO DO: Get path from user
		}

		// Compiler file header

		return false;
	}

	// Update Document
	bool OdDocument::updateDocument()
	{
		// Write file header to file

		std::wofstream outputFile("output.txt"); // Open the file for writing

		std::vector<wchar_t> headerBuffer = compileFileHeader();
		std::vector<wchar_t> databaseBuffer = compileFileDatabase();

		if (outputFile.is_open())
		{
			// Write header to file
			for (wchar_t c : headerBuffer)
				outputFile << c;

			// Write database to file
			for (wchar_t c : databaseBuffer)
				outputFile << c;

			outputFile.close(); // Close the file
		}
		else {
			std::cout << "Unable to open the file." << std::endl;
		}


		return false;
	}

	// Get Document Database
	OdDrawingDb* OdDocument::getDatabase()
	{
		return &database;
	}

	// Get Document Active Layer
	OdLayer* OdDocument::getActiveLayer()
	{
		return activeLayer;
	}

	// Get Document Editor
	System::OdEditor* OdDocument::getEditor()
	{
		return editor;
	}

	std::vector<wchar_t> OdDocument::compileFileHeader()
	{
		// Create vector to store file header
		std::vector<wchar_t> fileHeader;

		// Add version to file header
		for (int i = 0; i < sizeof(version); i++)
			fileHeader.push_back(version[i]);

		for (int i = 0; i < 3; i++)
			fileHeader.push_back(' ');

		// Add Creation Date to file header
		std::string creationDateStr = std::to_string(creationDate);

		for (int i = 0; i < creationDateStr.length(); i++)
			fileHeader.push_back(creationDateStr[i]);


		//// Add Modified Date to file header
		std::string modifiedDateStr = std::to_string(modifiedDate);

		for (int i = 0; i < creationDateStr.length(); i++)
			fileHeader.push_back(modifiedDateStr[i]);

		// Add Author to file header
		for (int i = 0; i < 64; i++)
		{
			if (author.size() > i)
				fileHeader.push_back(author[i]);
			else
				fileHeader.push_back(' ');
		}

		// Add Company to file header
		for (int i = 0; i < 64; i++)
		{
			if (company.size() > i)
				fileHeader.push_back(company[i]);
			else
				fileHeader.push_back(' ');
		}

		// Add Comments to file header
		for (int i = 0; i < 256; i++)
		{
			if (comments.size() > i)
				fileHeader.push_back(comments[i]);
			else
				fileHeader.push_back(' ');
		}

		return fileHeader;
	}


	std::vector<wchar_t> OdDocument::compileFileDatabase()
	{
		// Create vector to store file DB objects 
		std::vector<wchar_t> dbObjects;

		// Get number of objects in database
		int objectCount = database.getRecordCount();

		// For each object in the database
		for (int i = 0; i < objectCount; i++)
		{
			// Get object
			OdDbObject* object = database.getRecordByIndex(i);

			if (object == nullptr)
				continue;

			// Serialise object to ostream
			std::wstring stringBuffer;
			object->serialise(stringBuffer);
			
			// Add object to file DB objects
			for (int i = 0; i < stringBuffer.size(); i++)
				dbObjects.push_back(stringBuffer[i]);

		}

		return dbObjects;
	}


}// namespace OD::System

#endif // !OD_SY_DOCUMENT_H
