#ifndef OD_DAT_DOCUMENTMANAGER_CPP
#define OD_DAT_DOCUMENTMANAGER_CPP

/**************************************************************************************
* OpenDraft:    Document Manager Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDocumentManager.cpp                                                 *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document Manager class for OpenDraft framework.                                   *
***************************************************************************************/


#include "DataManager/OdDocumentManager.h"


namespace OD::Data
{
	// Constructors & Destructors
	OdDocumentManager::OdDocumentManager() : documents(), activeDocumentIndex(-1)
	{

	}

	OdDocumentManager::~OdDocumentManager()
	{
		// Delete all documents
		for (int i = 0; i < documents.size(); i++)
			delete documents[i];

	}


	//
	// Methods
	//
	
	bool OdDocumentManager::openDocument(const char* aFilepath)
	{
		// Check if file exists
		if (!std::filesystem::exists(aFilepath))
			return false;
		
		// Open a document
		OdDocument* doc = new OdDocument();
		
		// Set filepath of document
		doc->setPath(aFilepath);

		// Add document to manager
		documents.push_back(doc);
	}

	void OdDocumentManager::closeDocument(OdDocument* aDocument)
	{
		// Find document index
		for (int i = 0; i < documents.size(); i++)
		{
			if (documents[i] == aDocument)
			{
				// Close document
				//documents[i]->close();  //// TODO: Implement close method
				return;
			}
		}
	}

	void OdDocumentManager::addDocument(OdDocument* aDocument)
	{
		// Add document
		documents.push_back(aDocument);
	}

	void OdDocumentManager::removeDocument(OdDocument* aDocument)
	{
		// Find document index
		for (int i = 0; i < documents.size(); i++)
		{
			if (documents[i] == aDocument)
			{
				// Remove document
				documents.erase(documents.begin() + i);
				return;
			}
		}
	}

	void OdDocumentManager::removeDocument(int aIndex)
	{
		// Check if index is valid
		if (aIndex < 0 || aIndex >= documents.size())
			return;

		// Remove document
		documents.erase(documents.begin() + aIndex);
	}

	void OdDocumentManager::setActiveDocument(OdDocument* aDocument)
	{
		// Find document index
		for (int i = 0; i < documents.size(); i++)
		{
			if (documents[i] == aDocument)
			{
				activeDocumentIndex = i;
				return;
			}
		}
	}

	void OdDocumentManager::setActiveDocument(int aIndex)
	{
		activeDocumentIndex = aIndex;
	}

	OdDocument* OdDocumentManager::getActiveDocument()
	{
		return documents[activeDocumentIndex];
	}

	OdDocument* OdDocumentManager::getDocument(int aIndex)
	{
		// Check if index is valid
		if (aIndex < 0 || aIndex >= documents.size())
			return nullptr;
		
		// Return document
		return documents[aIndex];
	}

	int OdDocumentManager::getDocumentCount()
	{
		return documents.size();
	}

} // namespace OD::Data

#endif // !OD_DAT_DOCUMENTMANAGER_CPP
